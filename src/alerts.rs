
#[derive(Debug)]
pub(crate) enum AlertLevel {
    Alarm,
    Critical,
    Warning,
    Info
}

#[derive(Debug)]
pub(crate) struct AlertInfo {
    pub source: &'static str,
    pub message: String,
    pub level: AlertLevel
}

pub(crate) async fn send_alert(alert: AlertInfo) -> () {
    println!("{alert:#?}");
}