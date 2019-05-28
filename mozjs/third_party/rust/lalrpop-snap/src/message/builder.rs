use grammar::parse_tree::Span;
use message::{Content, Message};
use message::indent::Indent;
use message::horiz::Horiz;
use message::styled::Styled;
use message::text::Text;
use message::vert::Vert;
use message::wrap::Wrap;
use style::Style;

pub struct MessageBuilder {
    span: Span,
    heading: Option<Box<Content>>,
    body: Option<Box<Content>>,
}

pub struct HeadingCharacter {
    message: MessageBuilder,
}

pub struct BodyCharacter {
    message: MessageBuilder,
}

impl MessageBuilder {
    pub fn new(span: Span) -> Self {
        MessageBuilder {
            span: span,
            heading: None,
            body: None,
        }
    }

    pub fn heading(self) -> Builder<HeadingCharacter> {
        Builder::new(HeadingCharacter { message: self })
    }

    pub fn body(self) -> Builder<BodyCharacter> {
        Builder::new(BodyCharacter { message: self })
    }

    pub fn end(self) -> Message {
        Message::new(
            self.span,
            self.heading.expect("never defined a heading"),
            self.body.expect("never defined a body"),
        )
    }
}

impl Character for HeadingCharacter {
    type End = MessageBuilder;

    fn end(mut self, items: Vec<Box<Content>>) -> MessageBuilder {
        assert!(
            self.message.heading.is_none(),
            "already defined a heading for this message"
        );
        self.message.heading = Some(Box::new(Vert::new(items, 1)));
        self.message
    }
}

impl Character for BodyCharacter {
    type End = MessageBuilder;

    fn end(mut self, items: Vec<Box<Content>>) -> MessageBuilder {
        assert!(
            self.message.body.is_none(),
            "already defined a body for this message"
        );
        self.message.body = Some(Box::new(Vert::new(items, 2)));
        self.message
    }
}

///////////////////////////////////////////////////////////////////////////
// Inline builder: Useful for constructing little bits of content: for
// example, converting an Example into something renderable. Using an
// inline builder, if you push exactly one item, then when you call
// `end` that is what you get; otherwise, you get items laid out
// adjacent to one another horizontally (no spaces in between).

pub struct InlineBuilder;

impl InlineBuilder {
    pub fn new() -> Builder<InlineBuilder> {
        Builder::new(InlineBuilder)
    }
}

impl Character for InlineBuilder {
    type End = Box<Content>;

    fn end(self, mut items: Vec<Box<Content>>) -> Box<Content> {
        if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Box::new(Horiz::new(items, 1))
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// Builder -- generic helper for multi-part items

/// The builder is used to construct multi-part items. It is intended
/// to be used in a "method-call chain" style. The base method is
/// called `push`, and it simply pushes a new child of the current
/// parent.
///
/// Methods whose name like `begin_foo` are used to create a new
/// multi-part child; they return a fresh builder corresponding to the
/// child. When the child is completely constructed, call `end` to
/// finish the child builder and return to the parent builder.
///
/// Methods whose name ends in "-ed", such as `styled`, post-process
/// the last item pushed. They will panic if invoked before any items
/// have been pushed.
///
/// Example:
///
/// ```
/// let node =
///    InlineBuilder::new()
///        .begin_lines() // starts a child builder for adjacent lines
///        .text("foo")   // add a text node "foo" to the child builder
///        .text("bar")   // add a text node "bar" to the child builder
///        .end()         // finish the lines builder, return to the parent
///        .end();        // finish the parent `InlineBuilder`, yielding up the
///                       // `lines` child that was pushed (see `InlineBuilder`
///                       // for more details)
/// ```
pub struct Builder<C: Character> {
    items: Vec<Box<Content>>,
    character: C,
}

impl<C: Character> Builder<C> {
    fn new(character: C) -> Self {
        Builder {
            items: vec![],
            character: character,
        }
    }

    pub fn push<I>(mut self, item: I) -> Self
    where
        I: Into<Box<Content>>,
    {
        self.items.push(item.into());
        self
    }

    fn pop(&mut self) -> Option<Box<Content>> {
        self.items.pop()
    }

    pub fn begin_vert(self, separate: usize) -> Builder<VertCharacter<C>> {
        Builder::new(VertCharacter {
            base: self,
            separate: separate,
        })
    }

    pub fn begin_lines(self) -> Builder<VertCharacter<C>> {
        self.begin_vert(1)
    }

    pub fn begin_paragraphs(self) -> Builder<VertCharacter<C>> {
        self.begin_vert(2)
    }

    pub fn begin_horiz(self, separate: usize) -> Builder<HorizCharacter<C>> {
        Builder::new(HorizCharacter {
            base: self,
            separate: separate,
        })
    }

    // "item1item2"
    pub fn begin_adjacent(self) -> Builder<HorizCharacter<C>> {
        self.begin_horiz(1)
    }

    // "item1 item2"
    pub fn begin_spaced(self) -> Builder<HorizCharacter<C>> {
        self.begin_horiz(2)
    }

    pub fn begin_wrap(self) -> Builder<WrapCharacter<C>> {
        Builder::new(WrapCharacter { base: self })
    }

    pub fn styled(mut self, style: Style) -> Self {
        let content = self.pop().expect("bold must be applied to an item");
        self.push(Box::new(Styled::new(style, content)))
    }

    pub fn indented_by(mut self, amount: usize) -> Self {
        let content = self.pop().expect("indent must be applied to an item");
        self.push(Box::new(Indent::new(amount, content)))
    }

    pub fn indented(self) -> Self {
        self.indented_by(2)
    }

    pub fn text<T: ToString>(self, text: T) -> Self {
        self.push(Box::new(Text::new(text.to_string())))
    }

    /// Take the item just pushed and makes some text adjacent to it.
    /// E.g. `builder.wrap().text("foo").adjacent_text(".").end()`
    /// result in `"foo."` being printed without any wrapping in
    /// between.
    pub fn adjacent_text<T: ToString, U: ToString>(mut self, prefix: T, suffix: U) -> Self {
        let item = self.pop().expect("adjacent text must be added to an item");
        let prefix = prefix.to_string();
        let suffix = suffix.to_string();
        if !prefix.is_empty() && !suffix.is_empty() {
            self.begin_adjacent()
                .text(prefix)
                .push(item)
                .text(suffix)
                .end()
        } else if !suffix.is_empty() {
            self.begin_adjacent().push(item).text(suffix).end()
        } else if !prefix.is_empty() {
            self.begin_adjacent().text(prefix).push(item).end()
        } else {
            self.push(item)
        }
    }

    pub fn verbatimed(self) -> Self {
        self.adjacent_text("`", "`")
    }

    pub fn punctuated<T: ToString>(self, text: T) -> Self {
        self.adjacent_text("", text)
    }

    pub fn wrap_text<T: ToString>(self, text: T) -> Self {
        self.begin_wrap().text(text).end()
    }

    pub fn end(self) -> C::End {
        self.character.end(self.items)
    }
}

pub trait Character {
    type End;
    fn end(self, items: Vec<Box<Content>>) -> Self::End;
}

///////////////////////////////////////////////////////////////////////////

pub struct HorizCharacter<C: Character> {
    base: Builder<C>,
    separate: usize,
}

impl<C: Character> Character for HorizCharacter<C> {
    type End = Builder<C>;

    fn end(self, items: Vec<Box<Content>>) -> Builder<C> {
        self.base.push(Box::new(Horiz::new(items, self.separate)))
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct VertCharacter<C: Character> {
    base: Builder<C>,
    separate: usize,
}

impl<C: Character> Character for VertCharacter<C> {
    type End = Builder<C>;

    fn end(self, items: Vec<Box<Content>>) -> Builder<C> {
        self.base.push(Box::new(Vert::new(items, self.separate)))
    }
}

///////////////////////////////////////////////////////////////////////////

pub struct WrapCharacter<C: Character> {
    base: Builder<C>,
}

impl<C: Character> Character for WrapCharacter<C> {
    type End = Builder<C>;

    fn end(self, items: Vec<Box<Content>>) -> Builder<C> {
        self.base.push(Box::new(Wrap::new(items)))
    }
}

impl<T> From<Box<T>> for Box<Content>
where
    T: Content + 'static,
{
    fn from(b: Box<T>) -> Box<Content> {
        b
    }
}
