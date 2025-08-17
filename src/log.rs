use ansi_term::Colour;
use tracing_core::Event;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

struct CustomFormatter;

impl<S, N> FormatEvent<S, N> for CustomFormatter
where
    S: tracing_core::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _: &tracing_subscriber::fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        // 获取日志级别
        let level = event.metadata().level();

        // 根据级别设置颜色
        let (level_style, msg_style) = match *level {
            tracing::Level::ERROR => (Colour::Red.bold(), Colour::Red.bold()),
            tracing::Level::WARN => (Colour::Yellow.bold(), Colour::Yellow.bold()),
            tracing::Level::INFO => (Colour::Green.bold(), Colour::Green.bold()),
            tracing::Level::DEBUG => (Colour::Blue.bold(), Colour::Blue.bold()),
            tracing::Level::TRACE => (Colour::Purple.bold(), Colour::Purple.bold()),
        };

        match *level {
            tracing::Level::INFO => {}
            tracing::Level::ERROR => {
                write!(writer, "{} ", level_style.paint(format!("❌")))?;
            }
            tracing::Level::WARN => {
                write!(writer, "{} ", level_style.paint(format!("⚠️ ")))?;
            }
            tracing::Level::DEBUG => {
                write!(writer, "{} ", level_style.paint(format!("🐛")))?;
            }
            tracing::Level::TRACE => {
                write!(writer, "{} ", level_style.paint(format!("🔍")))?;
            }
        }

        // 格式化消息字段
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        // 输出带颜色的消息
        if let Some(message) = visitor.message {
            write!(writer, "{}", msg_style.paint(message))?;
        }

        writeln!(writer)
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{:?}", value));
        }
    }
}

pub fn init_logging(verbose_level: u8) {
    let filter = match verbose_level {
        0 => "INFO",
        1 => "DEBUG",
        _ => "TRACE",
    };
    // 创建自定义格式化层
    let formatting_layer = tracing_subscriber::fmt::layer()
        .event_format(CustomFormatter)
        .with_ansi(true);

    // 初始化订阅者
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)))
        .with(formatting_layer)
        .init();
}

#[cfg(test)]
mod tests {
    use colored::Colorize;

    use super::*;

    #[test]
    fn test_log() {
        init_logging(1);

        tracing::info!("test {} {}", "info".yellow(), "info".blue());
        tracing::debug!("test {}", 123);
        tracing::trace!("test {}", 123);
        tracing::warn!("test {}", 123);
        tracing::error!("test {}", 123);
    }
}
