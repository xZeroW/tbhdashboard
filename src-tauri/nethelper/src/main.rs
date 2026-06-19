#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("tbhdashboard-nethelper is Windows-only and requires WinDivert.");
    std::process::exit(2);
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(err) = windows::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::{
        collections::HashMap,
        ffi::{CString, c_char, c_void},
        fs,
        io::{Read, Write},
        mem,
        net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
        path::PathBuf,
        process::Command,
        ptr,
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    use windows_sys::Win32::{
        Foundation::{GetLastError, HANDLE, INVALID_HANDLE_VALUE},
        System::LibraryLoader::{GetProcAddress, LoadLibraryA},
    };

    const WINDIVERT_LAYER_NETWORK: i32 = 0;
    const MAX_PACKET_SIZE: usize = 0xFFFF;
    const ERROR_ACCESS_DENIED: u32 = 5;
    const ERROR_FILE_NOT_FOUND: u32 = 2;
    const ERROR_MOD_NOT_FOUND: u32 = 126;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const AF_INET: u32 = 2;
    const TCP_TABLE_OWNER_PID_ALL: u32 = 5;
    const TARGET_PORT: u16 = 443;
    const FLOW_TTL: Duration = Duration::from_secs(120);

    type WinDivertOpenFn = unsafe extern "system" fn(*const c_char, i32, i16, u64) -> HANDLE;
    type WinDivertRecvFn =
        unsafe extern "system" fn(HANDLE, *mut c_void, u32, *mut u32, *mut c_void) -> i32;
    type WinDivertSendFn =
        unsafe extern "system" fn(HANDLE, *const c_void, u32, *mut u32, *const c_void) -> i32;
    type WinDivertCloseFn = unsafe extern "system" fn(HANDLE) -> i32;
    type WinDivertHelperCalcChecksumsFn =
        unsafe extern "system" fn(*mut c_void, u32, *mut c_void, u64) -> u64;

    #[derive(Clone, Copy)]
    struct WinDivertApi {
        open: WinDivertOpenFn,
        recv: WinDivertRecvFn,
        send: WinDivertSendFn,
        close: WinDivertCloseFn,
        calc_checksums: WinDivertHelperCalcChecksumsFn,
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    struct FlowKey {
        client_ip: Ipv4Addr,
        client_port: u16,
        remote_ip: Ipv4Addr,
        remote_port: u16,
    }

    #[derive(Clone, Copy, Debug)]
    struct FlowTarget {
        remote_ip: Ipv4Addr,
        remote_port: u16,
        seen_at: Instant,
    }

    #[derive(Clone, Copy, Debug)]
    struct PacketInfo {
        ip_header_len: usize,
        src_ip: Ipv4Addr,
        dst_ip: Ipv4Addr,
        src_port: u16,
        dst_port: u16,
        syn: bool,
    }

    type FlowMap = Arc<Mutex<HashMap<FlowKey, FlowTarget>>>;

    pub fn run() -> Result<(), String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        match args.first().map(String::as_str) {
            Some("start") => {
                let game_pid = required_arg(&args, "--pid")?.parse::<u32>().map_err(|_| {
                    "Invalid --pid value for tbhdashboard-nethelper start".to_string()
                })?;
                let proxy = required_arg(&args, "--proxy")?
                    .parse::<SocketAddr>()
                    .map_err(|_| "Invalid --proxy value; expected host:port".to_string())?;
                let parent_pid = optional_arg(&args, "--parent")
                    .and_then(|value| value.parse::<u32>().ok());
                start(game_pid, parent_pid, proxy)
            }
            Some("stop") => stop(),
            Some("status") => status(),
            _ => Err("Usage: tbhdashboard-nethelper start --pid <game_pid> --proxy 127.0.0.1:8080 | stop | status".to_string()),
        }
    }

    fn start(game_pid: u32, parent_pid: Option<u32>, proxy: SocketAddr) -> Result<(), String> {
        let api = load_windivert()?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|err| format!("Failed to bind local transparent adapter listener: {err}"))?;
        let listener_port = listener
            .local_addr()
            .map_err(|err| format!("Failed to read adapter listener address: {err}"))?
            .port();

        write_state()?;

        let flows: FlowMap = Arc::new(Mutex::new(HashMap::new()));
        let bridge_flows = flows.clone();
        thread::spawn(move || accept_bridge(listener, bridge_flows, proxy));
        thread::spawn(move || monitor_lifetime(game_pid, parent_pid));

        eprintln!(
            "[TBH-nethelper] started for taskbarhero.exe pid {game_pid}; proxy={proxy}; adapter=127.0.0.1:{listener_port}"
        );
        divert_loop(api, game_pid, listener_port, flows)
    }

    fn monitor_lifetime(game_pid: u32, parent_pid: Option<u32>) {
        loop {
            thread::sleep(Duration::from_secs(2));
            if !pid_running(game_pid) || parent_pid.is_some_and(|pid| !pid_running(pid)) {
                eprintln!("[TBH-nethelper] game/dashboard exited; stopping helper");
                let _ = fs::remove_file(state_path());
                std::process::exit(0);
            }
        }
    }

    fn divert_loop(
        api: WinDivertApi,
        game_pid: u32,
        listener_port: u16,
        flows: FlowMap,
    ) -> Result<(), String> {
        let filter = format!(
            "outbound and ip and tcp and !impostor and (tcp.DstPort == {TARGET_PORT} or tcp.SrcPort == {listener_port})"
        );
        let filter = CString::new(filter).unwrap();
        let handle = unsafe { (api.open)(filter.as_ptr(), WINDIVERT_LAYER_NETWORK, 0, 0) };
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return Err(windivert_open_error(unsafe { GetLastError() }));
        }

        let mut packet = vec![0u8; MAX_PACKET_SIZE];
        let mut addr = [0u8; 128];

        loop {
            let mut packet_len = 0u32;
            let ok = unsafe {
                (api.recv)(
                    handle,
                    packet.as_mut_ptr().cast(),
                    packet.len() as u32,
                    &mut packet_len,
                    addr.as_mut_ptr().cast(),
                )
            };
            if ok == 0 {
                continue;
            }

            let len = packet_len as usize;
            let mut changed = false;
            if let Some(info) = parse_ipv4_tcp(&packet[..len]) {
                prune_flows(&flows);
                if info.src_port == listener_port {
                    changed = rewrite_adapter_packet(&mut packet[..len], info, &flows);
                } else if should_redirect(info, game_pid, &flows) {
                    changed = rewrite_game_packet(&mut packet[..len], info, listener_port, &flows);
                }
            }

            if changed {
                unsafe {
                    (api.calc_checksums)(
                        packet.as_mut_ptr().cast(),
                        packet_len,
                        addr.as_mut_ptr().cast(),
                        0,
                    );
                }
            }

            let mut sent_len = 0u32;
            unsafe {
                (api.send)(
                    handle,
                    packet.as_ptr().cast(),
                    packet_len,
                    &mut sent_len,
                    addr.as_ptr().cast(),
                );
            }
        }

        #[allow(unreachable_code)]
        unsafe {
            (api.close)(handle);
        }
        #[allow(unreachable_code)]
        Ok(())
    }

    fn should_redirect(info: PacketInfo, game_pid: u32, flows: &FlowMap) -> bool {
        if info.dst_port != TARGET_PORT || info.dst_ip.is_loopback() {
            return false;
        }

        let key = FlowKey {
            client_ip: info.src_ip,
            client_port: info.src_port,
            remote_ip: info.dst_ip,
            remote_port: info.dst_port,
        };
        if flows.lock().unwrap().contains_key(&key) {
            return true;
        }

        info.syn && tcp_owner_matches(game_pid, key)
    }

    fn rewrite_game_packet(
        packet: &mut [u8],
        info: PacketInfo,
        listener_port: u16,
        flows: &FlowMap,
    ) -> bool {
        let key = FlowKey {
            client_ip: info.src_ip,
            client_port: info.src_port,
            remote_ip: info.dst_ip,
            remote_port: info.dst_port,
        };
        flows.lock().unwrap().insert(
            key,
            FlowTarget {
                remote_ip: info.dst_ip,
                remote_port: info.dst_port,
                seen_at: Instant::now(),
            },
        );

        packet[16..20].copy_from_slice(&Ipv4Addr::LOCALHOST.octets());
        write_u16(packet, info.ip_header_len + 2, listener_port);
        true
    }

    fn rewrite_adapter_packet(packet: &mut [u8], info: PacketInfo, flows: &FlowMap) -> bool {
        let Some(target) = find_target(flows, info.dst_ip, info.dst_port) else {
            return false;
        };

        packet[12..16].copy_from_slice(&target.remote_ip.octets());
        write_u16(packet, info.ip_header_len, target.remote_port);
        true
    }

    fn find_target(flows: &FlowMap, client_ip: Ipv4Addr, client_port: u16) -> Option<FlowTarget> {
        let mut guard = flows.lock().unwrap();
        guard
            .iter_mut()
            .find(|(key, _)| {
                key.client_port == client_port
                    && (key.client_ip == client_ip || client_ip.is_loopback())
            })
            .map(|(_, target)| {
                target.seen_at = Instant::now();
                *target
            })
    }

    fn prune_flows(flows: &FlowMap) {
        let now = Instant::now();
        flows
            .lock()
            .unwrap()
            .retain(|_, target| now.duration_since(target.seen_at) < FLOW_TTL);
    }

    fn accept_bridge(listener: TcpListener, flows: FlowMap, proxy: SocketAddr) {
        for accepted in listener.incoming() {
            match accepted {
                Ok(stream) => {
                    let target = stream.peer_addr().ok().and_then(|addr| match addr {
                        SocketAddr::V4(addr) => find_target(&flows, *addr.ip(), addr.port()),
                        SocketAddr::V6(_) => None,
                    });
                    match target {
                        Some(target) => {
                            thread::spawn(move || bridge_connect(stream, target, proxy));
                        }
                        None => {
                            eprintln!(
                                "[TBH-nethelper] accepted redirected socket without original destination"
                            );
                        }
                    }
                }
                Err(err) => eprintln!("[TBH-nethelper] adapter accept failed: {err}"),
            }
        }
    }

    fn bridge_connect(mut game: TcpStream, target: FlowTarget, proxy: SocketAddr) {
        let target_addr = SocketAddrV4::new(target.remote_ip, target.remote_port);
        let mut proxy_stream = match TcpStream::connect(proxy) {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("[TBH-nethelper] failed to connect to Hudsucker proxy {proxy}: {err}");
                return;
            }
        };

        let connect = format!(
            "CONNECT {target_addr} HTTP/1.1\r\nHost: {target_addr}\r\nProxy-Connection: keep-alive\r\n\r\n"
        );
        if let Err(err) = proxy_stream.write_all(connect.as_bytes()) {
            eprintln!("[TBH-nethelper] failed to send CONNECT {target_addr}: {err}");
            return;
        }

        if let Err(err) = read_connect_response(&mut proxy_stream) {
            eprintln!("[TBH-nethelper] CONNECT {target_addr} failed: {err}");
            return;
        }

        let mut game_to_proxy_game = match game.try_clone() {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("[TBH-nethelper] failed to clone game socket: {err}");
                return;
            }
        };
        let mut game_to_proxy_proxy = match proxy_stream.try_clone() {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("[TBH-nethelper] failed to clone proxy socket: {err}");
                return;
            }
        };

        let forward = thread::spawn(move || {
            let _ = std::io::copy(&mut game_to_proxy_game, &mut game_to_proxy_proxy);
            let _ = game_to_proxy_proxy.shutdown(Shutdown::Write);
        });
        let _ = std::io::copy(&mut proxy_stream, &mut game);
        let _ = game.shutdown(Shutdown::Write);
        let _ = forward.join();
    }

    fn read_connect_response(stream: &mut TcpStream) -> Result<(), String> {
        let mut response = Vec::new();
        let mut buf = [0u8; 1];
        while response.len() < 8192 {
            let read = stream
                .read(&mut buf)
                .map_err(|err| format!("failed to read proxy response: {err}"))?;
            if read == 0 {
                break;
            }
            response.push(buf[0]);
            if response.ends_with(b"\r\n\r\n") {
                break;
            }
        }

        let text = String::from_utf8_lossy(&response);
        if text.starts_with("HTTP/1.1 200") || text.starts_with("HTTP/1.0 200") {
            Ok(())
        } else {
            Err(text
                .lines()
                .next()
                .unwrap_or("empty proxy response")
                .to_string())
        }
    }

    fn parse_ipv4_tcp(packet: &[u8]) -> Option<PacketInfo> {
        if packet.len() < 40 || packet[0] >> 4 != 4 {
            return None;
        }
        let ip_header_len = ((packet[0] & 0x0f) as usize) * 4;
        if ip_header_len < 20 || packet.len() < ip_header_len + 20 || packet[9] != 6 {
            return None;
        }
        let flags = packet[ip_header_len + 13];
        Some(PacketInfo {
            ip_header_len,
            src_ip: Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15]),
            dst_ip: Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]),
            src_port: read_u16(packet, ip_header_len),
            dst_port: read_u16(packet, ip_header_len + 2),
            syn: flags & 0x02 != 0,
        })
    }

    fn read_u16(packet: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes([packet[offset], packet[offset + 1]])
    }

    fn write_u16(packet: &mut [u8], offset: usize, value: u16) {
        packet[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }

    fn tcp_owner_matches(game_pid: u32, key: FlowKey) -> bool {
        query_tcp_rows().into_iter().any(|row| {
            row.owning_pid == game_pid
                && row.local_addr == key.client_ip
                && row.local_port == key.client_port
                && row.remote_addr == key.remote_ip
                && row.remote_port == key.remote_port
        })
    }

    #[derive(Debug)]
    struct TcpRow {
        local_addr: Ipv4Addr,
        local_port: u16,
        remote_addr: Ipv4Addr,
        remote_port: u16,
        owning_pid: u32,
    }

    #[repr(C)]
    struct MibTcpRowOwnerPid {
        state: u32,
        local_addr: u32,
        local_port: u32,
        remote_addr: u32,
        remote_port: u32,
        owning_pid: u32,
    }

    #[link(name = "iphlpapi")]
    unsafe extern "system" {
        fn GetExtendedTcpTable(
            tcp_table: *mut c_void,
            size: *mut u32,
            order: i32,
            af: u32,
            table_class: u32,
            reserved: u32,
        ) -> u32;
    }

    fn query_tcp_rows() -> Vec<TcpRow> {
        let mut size = 0u32;
        let first = unsafe {
            GetExtendedTcpTable(
                ptr::null_mut(),
                &mut size,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if first != ERROR_INSUFFICIENT_BUFFER || size == 0 {
            return Vec::new();
        }

        let mut buffer = vec![0u8; size as usize];
        let status = unsafe {
            GetExtendedTcpTable(
                buffer.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if status != 0 || buffer.len() < mem::size_of::<u32>() {
            return Vec::new();
        }

        let count = u32::from_ne_bytes(buffer[0..4].try_into().unwrap()) as usize;
        let rows_ptr = unsafe { buffer.as_ptr().add(4).cast::<MibTcpRowOwnerPid>() };
        let available = (buffer.len() - 4) / mem::size_of::<MibTcpRowOwnerPid>();
        let count = count.min(available);

        (0..count)
            .filter_map(|idx| {
                let row = unsafe { rows_ptr.add(idx).read_unaligned() };
                Some(TcpRow {
                    local_addr: ipv4_from_windows(row.local_addr),
                    local_port: port_from_windows(row.local_port),
                    remote_addr: ipv4_from_windows(row.remote_addr),
                    remote_port: port_from_windows(row.remote_port),
                    owning_pid: row.owning_pid,
                })
            })
            .collect()
    }

    fn ipv4_from_windows(value: u32) -> Ipv4Addr {
        let bytes = value.to_ne_bytes();
        Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])
    }

    fn port_from_windows(value: u32) -> u16 {
        let bytes = value.to_ne_bytes();
        u16::from_be_bytes([bytes[0], bytes[1]])
    }

    fn load_windivert() -> Result<WinDivertApi, String> {
        let dll = unsafe { LoadLibraryA(b"WinDivert.dll\0".as_ptr()) };
        if dll.is_null() {
            return Err(windivert_load_error(unsafe { GetLastError() }));
        }

        unsafe {
            Ok(WinDivertApi {
                open: load_symbol(dll, b"WinDivertOpen\0")?,
                recv: load_symbol(dll, b"WinDivertRecv\0")?,
                send: load_symbol(dll, b"WinDivertSend\0")?,
                close: load_symbol(dll, b"WinDivertClose\0")?,
                calc_checksums: load_symbol(dll, b"WinDivertHelperCalcChecksums\0")?,
            })
        }
    }

    unsafe fn load_symbol<T>(dll: *mut c_void, name: &[u8]) -> Result<T, String> {
        let symbol = unsafe { GetProcAddress(dll, name.as_ptr()) };
        let Some(symbol) = symbol else {
            return Err(format!(
                "WinDivert.dll is missing required symbol {}",
                String::from_utf8_lossy(&name[..name.len().saturating_sub(1)])
            ));
        };
        Ok(unsafe { mem::transmute_copy(&symbol) })
    }

    fn windivert_load_error(code: u32) -> String {
        match code {
            ERROR_FILE_NOT_FOUND | ERROR_MOD_NOT_FOUND => "WinDivert.dll was not found. Copy WinDivert.dll and WinDivert64.sys from the WinDivert release next to tbhdashboard-nethelper.exe, then run the helper as Administrator.".to_string(),
            _ => format!("Failed to load WinDivert.dll (Windows error {code}). Copy WinDivert.dll/WinDivert64.sys next to the helper and verify security software is not blocking the driver."),
        }
    }

    fn windivert_open_error(code: u32) -> String {
        match code {
            ERROR_ACCESS_DENIED => "WinDivert requires Administrator rights. Start TaskBarHeroDashboard as Administrator, or run tbhdashboard-nethelper.exe start from an elevated terminal.".to_string(),
            ERROR_FILE_NOT_FOUND | ERROR_MOD_NOT_FOUND => "WinDivert driver could not be loaded. Ensure WinDivert64.sys is next to tbhdashboard-nethelper.exe and is not blocked by Windows or antivirus policy.".to_string(),
            _ => format!("Failed to open WinDivert handle (Windows error {code}). Administrator rights and a loadable WinDivert driver are required."),
        }
    }

    fn required_arg(args: &[String], name: &str) -> Result<String, String> {
        args.windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
            .ok_or_else(|| format!("Missing required argument {name}"))
    }

    fn optional_arg(args: &[String], name: &str) -> Option<String> {
        args.windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
    }

    fn pid_running(pid: u32) -> bool {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
            })
            .unwrap_or(false)
    }

    fn state_path() -> PathBuf {
        std::env::temp_dir().join("tbhdashboard-nethelper.pid")
    }

    fn write_state() -> Result<(), String> {
        fs::write(state_path(), std::process::id().to_string())
            .map_err(|err| format!("Failed to write helper status file: {err}"))
    }

    fn stop() -> Result<(), String> {
        let pid = fs::read_to_string(state_path())
            .map_err(|_| "tbhdashboard-nethelper is not running".to_string())?
            .trim()
            .to_string();
        let status = Command::new("taskkill")
            .args(["/PID", &pid, "/T", "/F"])
            .status()
            .map_err(|err| format!("Failed to run taskkill for helper pid {pid}: {err}"))?;
        if status.success() {
            let _ = fs::remove_file(state_path());
            println!("stopped tbhdashboard-nethelper pid {pid}");
            Ok(())
        } else {
            Err(format!(
                "taskkill failed for tbhdashboard-nethelper pid {pid}"
            ))
        }
    }

    fn status() -> Result<(), String> {
        match fs::read_to_string(state_path()) {
            Ok(pid) => {
                println!("tbhdashboard-nethelper pid {}", pid.trim());
                Ok(())
            }
            Err(_) => {
                println!("tbhdashboard-nethelper stopped");
                Ok(())
            }
        }
    }
}
