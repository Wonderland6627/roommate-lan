//! Windows Service host for RoommateNetworkService.

#![cfg(windows)]

use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use windows_service::define_windows_service;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use crate::config;
use crate::service::engine::NetworkEngine;
use crate::service::pipe;

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

pub fn run_service() -> Result<(), String> {
    service_dispatcher::start(config::SERVICE_NAME, ffi_service_main)
        .map_err(|e| format!("启动 Windows 服务调度失败: {e}"))
}

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = run_service_body() {
        eprintln!("RoommateNetworkService error: {e}");
    }
}

fn run_service_body() -> Result<(), String> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_handler = Arc::clone(&stop_flag);

    let event_handler = move |control| -> ServiceControlHandlerResult {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stop_flag_handler.store(true, Ordering::SeqCst);
                // Unblock a waiting ConnectNamedPipe by opening the pipe as a client.
                let _ = crate::service::pipe::client_transact(&crate::service::protocol::Request {
                    v: crate::config::PROTOCOL_VERSION,
                    op: crate::service::protocol::Op::Health,
                    hostname: None,
                    ip: None,
                    login_server: None,
                    auth_key: None,
                });
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(config::SERVICE_NAME, event_handler)
        .map_err(|e| format!("注册服务控制句柄失败: {e}"))?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 1,
            wait_hint: Duration::from_secs(10),
            process_id: None,
        })
        .map_err(|e| format!("上报 StartPending 失败: {e}"))?;

    let engine = NetworkEngine::new()?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .map_err(|e| format!("上报 Running 失败: {e}"))?;

    // Accept loop + lease watchdog.
    let engine = Arc::new(engine);
    let engine_lease = Arc::clone(&engine);
    let stop_lease = Arc::clone(&stop_flag);
    std::thread::spawn(move || {
        while !stop_lease.load(Ordering::SeqCst) {
            engine_lease.enforce_lease();
            std::thread::sleep(Duration::from_secs(5));
        }
    });

    // Keep several acceptors so clients rarely hit a gap between RPC instances.
    const ACCEPTORS: usize = 3;
    let mut workers = Vec::with_capacity(ACCEPTORS);
    for _ in 0..ACCEPTORS {
        let engine_ref = Arc::clone(&engine);
        let stop = Arc::clone(&stop_flag);
        workers.push(std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let _ = pipe::serve_one(&engine_ref);
            }
        }));
    }

    while !stop_flag.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(400));
    }

    // Unblock ConnectNamedPipe waiters so acceptor threads can exit.
    for _ in 0..ACCEPTORS {
        let _ = pipe::client_transact(&crate::service::protocol::Request {
            v: crate::config::PROTOCOL_VERSION,
            op: crate::service::protocol::Op::Health,
            hostname: None,
            ip: None,
            login_server: None,
            auth_key: None,
        });
    }
    for w in workers {
        let _ = w.join();
    }

    engine.shutdown();

    status_handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .ok();

    Ok(())
}
