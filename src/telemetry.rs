use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};

pub fn config_logger<W>(name: String, level: String, writer: W)
where
    W: for<'a> MakeWriter<'a> + 'static + Send + Sync,
{
    // 创建一个 `tracing` 订阅者
    // 采用分层设计，先过滤，再格式化成JSON格式，最后输出

    // 过滤层
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    // 输出层
    let formatting_layer = BunyanFormattingLayer::new(
        name, // 将格式化的 `span` 输出到标准输出
        writer,
    );
    let subscriber = Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);

    // 将所有 `log` 的事件重定向到我们的订阅者
    LogTracer::init().expect("Failed to set logger");
    // `set_global_default` 可以由应用程序指定,应使用哪个订阅者来处理 `span`
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");
}
