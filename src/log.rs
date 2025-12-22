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

use std::borrow::Cow;

/// 样式部件枚举，存储样式信息而不是预格式化的字符串
#[derive(Debug, Clone)]
enum StylePart<'a> {
    /// 纯文本
    Text(Cow<'a, str>),
    /// 带颜色的文本
    Colored { text: Cow<'a, str>, color: Colour },
    /// 带样式的文本
    Styled { text: Cow<'a, str>, style: Style },
    /// 带颜色和样式的文本
    ColoredStyled {
        text: Cow<'a, str>,
        color: Colour,
        style: Style,
    },
}

pub struct StyledText<'a> {
    parts: Vec<StylePart<'a>>,
    sep: &'a str,
}

// 更新宏定义：支持静态字符串和动态字符串
macro_rules! color_method {
    ($name:ident, $color:expr, $doc:expr) => {
        #[doc = $doc]
        pub fn $name(&mut self, text: impl Into<Cow<'a, str>>) -> &mut Self {
            self.parts.push(StylePart::Colored {
                text: text.into(),
                color: $color,
            });
            self
        }
    };
}

macro_rules! style_method {
    ($name:ident, $style:expr, $doc:expr) => {
        #[doc = $doc]
        pub fn $name(&mut self, text: impl Into<Cow<'a, str>>) -> &mut Self {
            self.parts.push(StylePart::Styled {
                text: text.into(),
                style: $style,
            });
            self
        }
    };
}

macro_rules! color_style_method {
    ($name:ident, $color:expr, $style:expr, $doc:expr) => {
        #[doc = $doc]
        pub fn $name(&mut self, text: impl Into<Cow<'a, str>>) -> &mut Self {
            self.parts.push(StylePart::ColoredStyled {
                text: text.into(),
                color: $color,
                style: $style,
            });
            self
        }
    };
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
        self.parts
            .iter()
            .map(|part| match part {
                StylePart::Text(text) => text.to_string(),
                StylePart::Colored { text, color } => color.paint(text.as_ref()).to_string(),
                StylePart::Styled { text, style } => style.paint(text.as_ref()).to_string(),
                StylePart::ColoredStyled { text, color, style } => {
                    style.fg(*color).paint(text.as_ref()).to_string()
                }
            })
            .collect::<Vec<String>>()
            .join(self.sep)
    }

    pub fn build_bold(&self) -> String {
        self.parts
            .iter()
            .map(|part| match part {
                StylePart::Text(text) => Style::new().bold().paint(text.as_ref()).to_string(),
                StylePart::Colored { text, color } => Style::new()
                    .bold()
                    .fg(*color)
                    .paint(text.as_ref())
                    .to_string(),
                StylePart::Styled { text, style } => style.bold().paint(text.as_ref()).to_string(),
                StylePart::ColoredStyled { text, color, style } => {
                    style.bold().fg(*color).paint(text.as_ref()).to_string()
                }
            })
            .collect::<Vec<String>>()
            .join(self.sep)
    }

    /// 接收闭包
    pub fn with(&mut self, closure: impl FnOnce(&mut Self)) -> &mut Self {
        closure(self);
        self
    }

    pub fn text(&mut self, text: impl Into<Cow<'a, str>>) -> &mut Self {
        self.parts.push(StylePart::Text(text.into()));
        self
    }

    // 基本颜色方法
    color_method!(white, Colour::White, "白色");
    color_method!(red, Colour::Red, "红色");
    color_method!(green, Colour::Green, "绿色");
    color_method!(blue, Colour::Blue, "蓝色");
    color_method!(purple, Colour::Purple, "紫色");
    color_method!(yellow, Colour::Yellow, "黄色");
    color_method!(cyan, Colour::Cyan, "青色");
    color_method!(black, Colour::Black, "黑色");

    // 基本样式方法
    style_method!(bold, Style::new().bold(), "粗体");
    style_method!(dimmed, Style::new().dimmed(), "淡化");
    style_method!(italic, Style::new().italic(), "斜体");
    style_method!(underline, Style::new().underline(), "下划线");
    style_method!(blink, Style::new().blink(), "闪烁");
    style_method!(reverse, Style::new().reverse(), "反色");
    style_method!(hidden, Style::new().hidden(), "隐藏");
    style_method!(strikethrough, Style::new().strikethrough(), "删除线");

    // 颜色+粗体组合
    color_style_method!(white_bold, Colour::White, Style::new().bold(), "白色粗体");
    color_style_method!(red_bold, Colour::Red, Style::new().bold(), "红色粗体");
    color_style_method!(green_bold, Colour::Green, Style::new().bold(), "绿色粗体");
    color_style_method!(blue_bold, Colour::Blue, Style::new().bold(), "蓝色粗体");
    color_style_method!(purple_bold, Colour::Purple, Style::new().bold(), "紫色粗体");
    color_style_method!(yellow_bold, Colour::Yellow, Style::new().bold(), "黄色粗体");
    color_style_method!(cyan_bold, Colour::Cyan, Style::new().bold(), "青色粗体");
    color_style_method!(black_bold, Colour::Black, Style::new().bold(), "黑色粗体");

    // 颜色+下划线组合
    color_style_method!(
        white_underline,
        Colour::White,
        Style::new().underline(),
        "白色下划线"
    );
    color_style_method!(
        red_underline,
        Colour::Red,
        Style::new().underline(),
        "红色下划线"
    );
    color_style_method!(
        green_underline,
        Colour::Green,
        Style::new().underline(),
        "绿色下划线"
    );
    color_style_method!(
        blue_underline,
        Colour::Blue,
        Style::new().underline(),
        "蓝色下划线"
    );
    color_style_method!(
        purple_underline,
        Colour::Purple,
        Style::new().underline(),
        "紫色下划线"
    );
    color_style_method!(
        yellow_underline,
        Colour::Yellow,
        Style::new().underline(),
        "黄色下划线"
    );
    color_style_method!(
        cyan_underline,
        Colour::Cyan,
        Style::new().underline(),
        "青色下划线"
    );
    color_style_method!(
        black_underline,
        Colour::Black,
        Style::new().underline(),
        "黑色下划线"
    );

    // RGB 颜色方法
    pub fn rgb(&mut self, r: u8, g: u8, b: u8, text: impl Into<Cow<'a, str>>) -> &mut Self {
        self.parts.push(StylePart::Colored {
            text: text.into(),
            color: Colour::RGB(r, g, b),
        });
        self
    }

    pub fn rgb_bold(&mut self, r: u8, g: u8, b: u8, text: impl Into<Cow<'a, str>>) -> &mut Self {
        self.parts.push(StylePart::ColoredStyled {
            text: text.into(),
            color: Colour::RGB(r, g, b),
            style: Style::new().bold(),
        });
        self
    }

    // 固定颜色编号方法
    pub fn fixed(&mut self, color_num: u8, text: impl Into<Cow<'a, str>>) -> &mut Self {
        self.parts.push(StylePart::Colored {
            text: text.into(),
            color: Colour::Fixed(color_num),
        });
        self
    }

    pub fn fixed_bold(&mut self, color_num: u8, text: impl Into<Cow<'a, str>>) -> &mut Self {
        self.parts.push(StylePart::ColoredStyled {
            text: text.into(),
            color: Colour::Fixed(color_num),
            style: Style::new().bold(),
        });
        self
    }

    /// 直接输出到终端，避免字符串分配
    pub fn print(&self) {
        let mut first = true;
        for part in &self.parts {
            if !first {
                print!("{}", self.sep);
            }
            first = false;

            match part {
                StylePart::Text(text) => print!("{}", text),
                StylePart::Colored { text, color } => print!("{}", color.paint(text.as_ref())),
                StylePart::Styled { text, style } => print!("{}", style.paint(text.as_ref())),
                StylePart::ColoredStyled { text, color, style } => {
                    print!("{}", style.fg(*color).paint(text.as_ref()))
                }
            }
        }
        println!();
    }

    /// 获取部件数量
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// 清空所有部件
    pub fn clear(&mut self) {
        self.parts.clear();
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
            .text("plain")
            .text("owned".to_string()) // 测试 String 输入
            .white("white")
            .red("red")
            .green("green")
            .blue("blue")
            .purple("purple")
            .yellow("yellow")
            .cyan("cyan")
            .black("black")
            .bold("bold")
            .dimmed("dimmed")
            .italic("italic")
            .underline("underline")
            .blink("blink")
            .reverse("reverse")
            .hidden("hidden")
            .strikethrough("strikethrough")
            .white_bold("white_bold")
            .red_bold("red_bold")
            .green_bold("green_bold")
            .blue_bold("blue_bold")
            .purple_bold("purple_bold")
            .yellow_bold("yellow_bold")
            .cyan_bold("cyan_bold")
            .black_bold("black_bold")
            .white_underline("white_underline")
            .red_underline("red_underline")
            .green_underline("green_underline")
            .blue_underline("blue_underline")
            .purple_underline("purple_underline")
            .yellow_underline("yellow_underline")
            .cyan_underline("cyan_underline")
            .black_underline("black_underline")
            .rgb(255, 100, 100, "rgb_pink")
            .rgb_bold(100, 255, 100, "rgb_green_bold")
            .fixed(202, "fixed_orange")
            .fixed_bold(45, "fixed_blue_bold")
            .with(|t| {
                t.green("with_closure");
            });

        assert!(!styled_text.is_empty());
        assert!(styled_text.len() > 0);

        // 测试直接输出方法
        styled_text.println();
        println!();
        styled_text.println_bold();

        styled_text.clear();
        assert!(styled_text.is_empty());
        assert_eq!(styled_text.len(), 0);
    }
}
