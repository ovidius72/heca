use super::Style;

/// A kind that holds children — implemented only for the kinds `realize` attaches them to.
///
/// A `Toast` draws its own card and takes none, so it has no `child`. That is the same rule as the
/// properties: if the widget cannot do it, the builder cannot say it.
pub trait Parent: Style {
    /// **Append a child, or several** — one node, or a `Vec`/array of them. One door, the same as
    /// `ViewNode::child` and the native `Parent::child`.
    fn child(mut self, children: impl crate::IntoNodes) -> Self {
        self.node_mut().children.extend(children.into_nodes());
        self
    }
}

/// What every builder is, whatever it takes to construct: a [`Style`] over its node, and a way back
/// into the [`ViewNode`]. Written once, so a builder made by either macro below cannot differ.
macro_rules! builder_impls {
    ($name:ident) => {
        impl Style for $name {
            fn node_mut(&mut self) -> &mut ViewNode {
                &mut self.0
            }
            fn into_node(self) -> ViewNode {
                self.0
            }
        }

        impl From<$name> for ViewNode {
            fn from(b: $name) -> ViewNode {
                b.0
            }
        }
    };
}

macro_rules! builder {
    ($(#[$m:meta])* $name:ident => $kind:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub(super) ViewNode);

        impl $name {
            /// A new node of this kind.
            pub fn new() -> Self {
                Self(ViewNode::new(WidgetKind::$kind))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        builder_impls!($name);
    };
}

/// The same, for kinds whose **whole content is the scalar text** — the text goes in the
/// constructor, because a `Label` with no text is not a thing anyone means to write.
macro_rules! builder_text {
    ($(#[$m:meta])* $name:ident => $kind:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub(super) ViewNode);

        impl $name {
            /// A new node of this kind, showing `text`.
            pub fn new(text: impl Into<String>) -> Self {
                Self(ViewNode::new(WidgetKind::$kind).text(text))
            }
        }

        builder_impls!($name);
    };
}

/// Give a builder the scalar `text` sugar, for the kinds whose arm reads it.
macro_rules! with_text {
    ($($name:ident),* $(,)?) => {
        $(
            impl $name {
                /// The scalar text this kind shows.
                pub fn text(mut self, text: impl Into<String>) -> Self {
                    self.0.props.insert("text".into(), PropValue::Text(text.into()));
                    self
                }
            }
        )*
    };
}

/// Give a builder an event binding, for the events `realize` reads on that kind.
macro_rules! with_event {
    ($($name:ident { $($method:ident => $event:ident),* $(,)? })*) => {
        $(
            impl $name {
                $(
                    #[doc = concat!("Bind [`ViewEvent::", stringify!($event), "`] to an intent.")]
                    pub fn $method(mut self, intent: Intent) -> Self {
                        self.0.events.insert(ViewEvent::$event.name().to_string(), intent);
                        self
                    }
                )*
            }
        )*
    };
}
