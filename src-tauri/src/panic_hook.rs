use std::panic;
use std::process;

pub fn set_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        default_hook(info);

        let location = info
            .location()
            .map_or_else(|| "unknown location".to_string(), |loc| loc.to_string());

        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        log::error!("PANIC at {}: {}", location, msg);
        process::abort();
    }));
}
