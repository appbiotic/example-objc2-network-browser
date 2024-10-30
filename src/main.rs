#![allow(non_camel_case_types)]
#![allow(clippy::missing_safety_doc)]

use std::{
    ffi::{self, CStr, CString},
    process::ExitCode,
    ptr::{self, NonNull},
};

use block2::{Block, RcBlock};
use dispatch::ffi::{dispatch_get_global_queue, dispatch_queue_t, DISPATCH_QUEUE_PRIORITY_DEFAULT};
use objc2::{
    extern_protocol,
    rc::Retained,
    runtime::{Bool, NSObjectProtocol, ProtocolObject},
    ProtocolType,
};
use tokio::signal;

extern_protocol!(
    pub unsafe trait OS_nw_browse_result: NSObjectProtocol {}
    unsafe impl ProtocolType for dyn OS_nw_browse_result {}
);
pub type nw_browser_result_t = ProtocolObject<dyn OS_nw_browse_result>;

extern_protocol!(
    pub unsafe trait OS_nw_browser: NSObjectProtocol {}
    unsafe impl ProtocolType for dyn OS_nw_browser {}
);
pub type nw_browser_t = ProtocolObject<dyn OS_nw_browser>;

extern_protocol!(
    pub unsafe trait OS_nw_endpoint: NSObjectProtocol {}
    unsafe impl ProtocolType for dyn OS_nw_endpoint {}
);
pub type nw_endpoint_t = ProtocolObject<dyn OS_nw_endpoint>;

extern_protocol!(
    pub unsafe trait OS_nw_parameters: NSObjectProtocol {}
    unsafe impl ProtocolType for dyn OS_nw_parameters {}
);
pub type nw_parameters_t = ProtocolObject<dyn OS_nw_parameters>;

extern_protocol!(
    pub unsafe trait OS_nw_browse_descriptor: NSObjectProtocol {}
    unsafe impl ProtocolType for dyn OS_nw_browse_descriptor {}
);
pub type nw_browse_descriptor_t = ProtocolObject<dyn OS_nw_browse_descriptor>;

#[link(name = "Network", kind = "framework")]
extern "C" {
    pub fn nw_browse_result_copy_endpoint(result: &nw_browser_result_t) -> *mut nw_endpoint_t;

    pub fn nw_browse_descriptor_create_application_service(
        application_service_name: *const ffi::c_char,
    ) -> *mut nw_browse_descriptor_t;

    pub fn nw_browse_descriptor_create_bonjour_service(
        type_: *const ffi::c_char,
        domain: *const ffi::c_char,
    ) -> *mut nw_browse_descriptor_t;

    pub fn nw_browser_create(
        descriptor: &nw_browse_descriptor_t,
        parameters: Option<&nw_parameters_t>,
    ) -> *mut nw_browser_t;

    pub fn nw_browser_set_browse_results_changed_handler(
        browser: &nw_browser_t,
        handler: Option<
            &Block<dyn Fn(*mut nw_browser_result_t, NonNull<nw_browser_result_t>, Bool)>,
        >,
    );

    pub fn nw_browser_set_queue(browser: &nw_browser_t, queue: dispatch_queue_t);

    pub fn nw_browser_start(browser: &nw_browser_t);

    pub fn nw_endpoint_get_bonjour_service_domain(endpoint: &nw_endpoint_t) -> *const ffi::c_char;

    pub fn nw_endpoint_get_bonjour_service_name(endpoint: &nw_endpoint_t) -> *const ffi::c_char;
}

unsafe fn string_from_raw_or_default(raw_ptr: *const ffi::c_char) -> String {
    if !raw_ptr.is_null() {
        CStr::from_ptr(raw_ptr).to_string_lossy().to_string()
    } else {
        String::from("[NULL]")
    }
}

#[tokio::main]
#[allow(clippy::unnecessary_literal_unwrap)]
async fn main() -> ExitCode {
    let service_type = "_http._tcp";
    let domain: Option<&str> = None;

    println!(
        "Browsing with service_type `{service_type}` domain: `{}`",
        domain.unwrap_or_default()
    );

    unsafe {
        let service_type_ = CString::new(service_type).unwrap();
        let domain_ = domain.map(|x| CString::new(x).unwrap());

        let browse_descriptor = Retained::from_raw(nw_browse_descriptor_create_bonjour_service(
            service_type_.as_ptr(),
            domain_.map(|x| x.as_ptr()).unwrap_or(ptr::null()),
        ))
        .unwrap();

        let browser = Retained::from_raw(nw_browser_create(&browse_descriptor, None)).unwrap();

        let f = |_prev: *mut nw_browser_result_t,
                 curr: NonNull<nw_browser_result_t>,
                 no_more: Bool| {
            let cur_endpoint =
                Retained::from_raw(nw_browse_result_copy_endpoint(curr.as_ref())).unwrap();
            let cur_endpoint_bonjour_service_name =
                string_from_raw_or_default(nw_endpoint_get_bonjour_service_name(&cur_endpoint));
            let cur_endpoint_bonjour_service_domain =
                string_from_raw_or_default(nw_endpoint_get_bonjour_service_domain(&cur_endpoint));

            println!(
                    "Event for service `{cur_endpoint_bonjour_service_name}` domain `{cur_endpoint_bonjour_service_domain}` expect more `{}`",
                    !no_more.as_raw()
                );
        };
        let handler = RcBlock::new(f);

        nw_browser_set_browse_results_changed_handler(&browser, Some(&handler));

        let queue = dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0);
        nw_browser_set_queue(&browser, queue);

        nw_browser_start(&browser);
    }

    println!("Browsing...");

    shutdown_signal().await;

    println!("Shutting down");

    ExitCode::SUCCESS
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.unwrap() };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
