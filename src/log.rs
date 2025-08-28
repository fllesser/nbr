use ansi_term::{Colour, Style};
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
        let msg_style = match *level {
            tracing::Level::ERROR => Colour::Red.bold(),
            tracing::Level::WARN => Colour::Yellow.bold(),
            tracing::Level::INFO => Colour::Green.bold(),
            tracing::Level::DEBUG => Colour::Blue.normal(),
            tracing::Level::TRACE => Colour::Purple.normal(),
        };

        match *level {
            tracing::Level::INFO => {}
            tracing::Level::ERROR => {
                write!(writer, "❌ ")?;
            }
            tracing::Level::WARN => {
                write!(writer, "⚠️  ")?;
            }
            tracing::Level::DEBUG => {
                write!(
                    writer,
                    "{} ",
                    Style::new().bold().fg(Colour::Blue).paint("[DEBUG]")
                )?;
            }
            tracing::Level::TRACE => {
                write!(
                    writer,
                    "{} ",
                    Style::new().bold().fg(Colour::Purple).paint("[TRACE]")
                )?;
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

#[allow(unused)]
pub struct StyledText<'a> {
    parts: Vec<String>,
    sep: &'a str,
}

#[allow(unused)]
impl<'a> StyledText<'a> {
    pub fn new(sep: &'a str) -> Self {
        let parts = Vec::new();
        Self { parts, sep }
    }

    pub fn println(&self) {
        println!("{}", self.build());
    }

    pub fn println_bold(&self) {
        println!("{}", self.build_bold());
    }

    pub fn build(&self) -> String {
        self.parts.join(self.sep)
    }

    pub fn build_bold(&self) -> String {
        self.parts
            .iter()
            .map(|part| Style::new().bold().paint(part).to_string())
            .collect::<Vec<String>>()
            .join(self.sep)
    }

    /// 接收闭包
    pub fn with(&mut self, closure: impl FnOnce(&mut Self)) -> &mut Self {
        closure(self);
        self
    }

    pub fn text(&mut self, text: &str) -> &mut Self {
        self.parts.push(text.to_string());
        self
    }

    pub fn white(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::White.paint(text).to_string());
        self
    }

    pub fn black(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::Black.paint(text).to_string());
        self
    }

    /// 红色
    pub fn red(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::Red.paint(text).to_string());
        self
    }

    /// 绿色
    pub fn green(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::Green.paint(text).to_string());
        self
    }

    /// 蓝色
    pub fn blue(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::Blue.paint(text).to_string());
        self
    }

    /// 紫色
    pub fn purple(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::Purple.paint(text).to_string());
        self
    }

    /// 黄色
    pub fn yellow(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::Yellow.paint(text).to_string());
        self
    }

    /// 青色
    pub fn cyan(&mut self, text: &str) -> &mut Self {
        self.parts.push(Colour::Cyan.paint(text).to_string());
        self
    }

    /// 粗体
    pub fn bold(&mut self, text: &str) -> &mut Self {
        self.parts.push(Style::new().bold().paint(text).to_string());
        self
    }

    /// 淡化
    pub fn dimmed(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().dimmed().paint(text).to_string());
        self
    }

    /// 斜体
    pub fn italic(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().italic().paint(text).to_string());
        self
    }

    /// 下划线
    pub fn underline(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().underline().paint(text).to_string());
        self
    }

    /// 闪烁
    pub fn blink(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().blink().paint(text).to_string());
        self
    }

    /// 反色
    pub fn reverse(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().reverse().paint(text).to_string());
        self
    }

    /// 隐藏
    pub fn hidden(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().hidden().paint(text).to_string());
        self
    }

    /// 删除线
    pub fn strikethrough(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().strikethrough().paint(text).to_string());
        self
    }

    /// 蓝色粗体
    pub fn blue_bold(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().bold().fg(Colour::Blue).paint(text).to_string());
        self
    }

    /// 青色粗体
    pub fn cyan_bold(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().bold().fg(Colour::Cyan).paint(text).to_string());
        self
    }

    /// 绿色粗体
    pub fn green_bold(&mut self, text: &str) -> &mut Self {
        self.parts.push(
            Style::new()
                .bold()
                .fg(Colour::Green)
                .paint(text)
                .to_string(),
        );
        self
    }

    /// 红色粗体
    pub fn red_bold(&mut self, text: &str) -> &mut Self {
        self.parts
            .push(Style::new().bold().fg(Colour::Red).paint(text).to_string());
        self
    }

    /// 黄色粗体
    pub fn yellow_bold(&mut self, text: &str) -> &mut Self {
        self.parts.push(
            Style::new()
                .bold()
                .fg(Colour::Yellow)
                .paint(text)
                .to_string(),
        );
        self
    }

    /// 紫色粗体
    pub fn purple_bold(&mut self, text: &str) -> &mut Self {
        self.parts.push(
            Style::new()
                .bold()
                .fg(Colour::Purple)
                .paint(text)
                .to_string(),
        );
        self
    }

    /// 白色粗体
    pub fn white_bold(&mut self, text: &str) -> &mut Self {
        self.parts.push(
            Style::new()
                .bold()
                .fg(Colour::White)
                .paint(text)
                .to_string(),
        );
        self
    }

    /// 黑色粗体
    pub fn black_bold(&mut self, text: &str) -> &mut Self {
        self.parts.push(
            Style::new()
                .bold()
                .fg(Colour::Black)
                .paint(text)
                .to_string(),
        );
        self
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_log() {
        init_logging(1);

        tracing::info!(
            "test {} {}",
            Colour::Yellow.paint("info"),
            Colour::Cyan.paint("info")
        );
        tracing::debug!("test {}", 123);
        tracing::trace!("test {}", 123);
        tracing::warn!("test {}", 123);
        tracing::error!("test {}", 123);
    }

    #[test]
    fn test_styled_text() {
        let mut styled_text = StyledText::new(" ");
        styled_text
            .red("red")
            .green("green")
            .blue("blue")
            .blue_bold("blue_bold")
            .purple("purple")
            .yellow("yellow")
            .cyan("cyan")
            .bold("bold")
            .cyan_bold("cyan_bold")
            .green_bold("green_bold")
            .red_bold("red_bold")
            .yellow_bold("yellow_bold")
            .purple_bold("purple_bold")
            .white_bold("white_bold")
            .black_bold("black_bold")
            .dimmed("dimmed")
            .italic("italic")
            .underline("underline")
            .blink("blink")
            .reverse("reverse")
            .hidden("hidden")
            .strikethrough("strikethrough");
        println!("{}", styled_text.build());
        println!("{}", styled_text.build_bold());
    }
}
