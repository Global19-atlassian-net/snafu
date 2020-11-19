use crate::SnafuAttribute;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{
    parenthesized,
    parse::{Parse, ParseBuffer, ParseStream, Result},
    punctuated::Punctuated,
    token, Expr, LitBool, LitStr, Path, Type,
};

mod kw {
    use syn::custom_keyword;

    custom_keyword!(backtrace);
    custom_keyword!(context);
    custom_keyword!(crate_root);
    custom_keyword!(display);
    custom_keyword!(whatever);
    custom_keyword!(source);
    custom_keyword!(visibility);

    custom_keyword!(delegate); // deprecated
    custom_keyword!(from);
}

pub(crate) fn attributes_from_syn(
    attrs: Vec<syn::Attribute>,
) -> super::MultiSynResult<Vec<SnafuAttribute>> {
    let mut ours = Vec::new();
    let mut errs = Vec::new();

    for attr in attrs {
        if attr.path.is_ident("snafu") {
            let attr_list = Punctuated::<Attribute, token::Comma>::parse_terminated;

            match attr.parse_args_with(attr_list) {
                Ok(attrs) => {
                    ours.extend(attrs.into_iter().map(Into::into));
                }
                Err(e) => errs.push(e),
            }
        } else if attr.path.is_ident("doc") {
            // Ignore any errors that occur while parsing the doc
            // comment. This isn't our attribute so we shouldn't
            // assume that we know what values are acceptable.
            if let Ok(comment) = syn::parse2::<DocComment>(attr.tokens) {
                ours.push(comment.into());
            }
        }
    }

    if errs.is_empty() {
        Ok(ours)
    } else {
        Err(errs)
    }
}

enum Attribute {
    Backtrace(Backtrace),
    Context(Context),
    CrateRoot(CrateRoot),
    Display(Display),
    Whatever(Whatever),
    Source(Source),
    Visibility(Visibility),
}

impl From<Attribute> for SnafuAttribute {
    fn from(other: Attribute) -> Self {
        use self::Attribute::*;

        match other {
            Backtrace(b) => SnafuAttribute::Backtrace(b.to_token_stream(), b.into_bool()),
            Context(c) => SnafuAttribute::Context(c.to_token_stream(), c.into_bool()),
            CrateRoot(cr) => SnafuAttribute::CrateRoot(cr.to_token_stream(), cr.into_arbitrary()),
            Display(d) => SnafuAttribute::Display(d.to_token_stream(), d.into_arbitrary()),
            Whatever(o) => Self::Whatever(o.to_token_stream()),
            Source(s) => SnafuAttribute::Source(s.to_token_stream(), s.into_components()),
            Visibility(v) => SnafuAttribute::Visibility(v.to_token_stream(), v.into_arbitrary()),
        }
    }
}

impl Parse for Attribute {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::backtrace) {
            input.parse().map(Attribute::Backtrace)
        } else if lookahead.peek(kw::context) {
            input.parse().map(Attribute::Context)
        } else if lookahead.peek(kw::crate_root) {
            input.parse().map(Attribute::CrateRoot)
        } else if lookahead.peek(kw::display) {
            input.parse().map(Attribute::Display)
        } else if lookahead.peek(kw::whatever) {
            input.parse().map(Attribute::Whatever)
        } else if lookahead.peek(kw::source) {
            input.parse().map(Attribute::Source)
        } else if lookahead.peek(kw::visibility) {
            input.parse().map(Attribute::Visibility)
        } else {
            Err(lookahead.error())
        }
    }
}

struct Backtrace {
    backtrace_token: kw::backtrace,
    arg: MaybeArg<BacktraceArg>,
}

impl Backtrace {
    fn into_bool(self) -> bool {
        self.arg.into_option().map_or(true, |a| a.value.value)
    }
}

impl Parse for Backtrace {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            backtrace_token: input.parse()?,
            arg: input.parse()?,
        })
    }
}

impl ToTokens for Backtrace {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.backtrace_token.to_tokens(tokens);
        self.arg.to_tokens(tokens);
    }
}

struct BacktraceArg {
    value: LitBool,
}

impl Parse for BacktraceArg {
    fn parse(input: ParseStream) -> Result<Self> {
        // TODO: Remove this with a semver-incompatible release
        if input.peek(kw::delegate) {
            return Err(input.error(
                "`backtrace(delegate)` has been removed; use `backtrace` on a source field",
            ));
        }

        Ok(Self {
            value: input.parse()?,
        })
    }
}

impl ToTokens for BacktraceArg {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.value.to_tokens(tokens);
    }
}

struct Context {
    context_token: kw::context,
    arg: MaybeArg<LitBool>,
}

impl Context {
    fn into_bool(self) -> bool {
        self.arg.into_option().map_or(true, |a| a.value)
    }
}

impl Parse for Context {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            context_token: input.parse()?,
            arg: input.parse()?,
        })
    }
}

impl ToTokens for Context {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.context_token.to_tokens(tokens);
        self.arg.to_tokens(tokens);
    }
}

struct CrateRoot {
    crate_root_token: kw::crate_root,
    arg: CompatArg<Path>,
}

impl CrateRoot {
    // TODO: Remove boxed trait object
    fn into_arbitrary(self) -> Box<dyn ToTokens> {
        Box::new(self.arg.into_value())
    }
}

impl Parse for CrateRoot {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            crate_root_token: input.parse()?,
            arg: input.parse()?,
        })
    }
}

impl ToTokens for CrateRoot {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.crate_root_token.to_tokens(tokens);
        self.arg.to_tokens(tokens);
    }
}

struct Display {
    display_token: kw::display,
    args: CompatArg<Punctuated<Expr, token::Comma>>,
}

impl Display {
    // TODO: Remove boxed trait object
    fn into_arbitrary(self) -> Box<dyn ToTokens> {
        Box::new(self.args.into_value())
    }
}

impl Parse for Display {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            display_token: input.parse()?,
            args: CompatArg::parse_with(&input, Punctuated::parse_terminated)?,
        })
    }
}

impl ToTokens for Display {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.display_token.to_tokens(tokens);
        self.args.to_tokens(tokens);
    }
}

struct DocComment {
    eq_token: token::Eq,
    str: LitStr,
}

impl DocComment {
    fn into_value(self) -> String {
        self.str.value()
    }
}

impl From<DocComment> for SnafuAttribute {
    fn from(other: DocComment) -> Self {
        SnafuAttribute::DocComment(other.to_token_stream(), other.into_value())
    }
}

impl Parse for DocComment {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            eq_token: input.parse()?,
            str: input.parse()?,
        })
    }
}

impl ToTokens for DocComment {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.eq_token.to_tokens(tokens);
        self.str.to_tokens(tokens);
    }
}

struct Whatever {
    whatever_token: kw::whatever,
}

impl Parse for Whatever {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            whatever_token: input.parse()?,
        })
    }
}

impl ToTokens for Whatever {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.whatever_token.to_tokens(tokens);
    }
}

struct Source {
    source_token: kw::source,
    args: MaybeArg<Punctuated<SourceArg, token::Comma>>,
}

impl Source {
    fn into_components(self) -> Vec<super::Source> {
        match self.args.into_option() {
            None => vec![super::Source::Flag(true)],
            Some(args) => args
                .into_iter()
                .map(|sa| match sa {
                    SourceArg::Flag { value } => super::Source::Flag(value.value),
                    SourceArg::From { r#type, expr, .. } => super::Source::From(r#type, expr),
                })
                .collect(),
        }
    }
}

impl Parse for Source {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            source_token: input.parse()?,
            args: MaybeArg::parse_with(&input, Punctuated::parse_terminated)?,
        })
    }
}

impl ToTokens for Source {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.source_token.to_tokens(tokens);
        self.args.to_tokens(tokens);
    }
}

enum SourceArg {
    Flag {
        value: LitBool,
    },
    From {
        from_token: kw::from,
        paren_token: token::Paren,
        r#type: Type,
        comma_token: token::Comma,
        expr: Expr,
    },
}

impl Parse for SourceArg {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(LitBool) {
            Ok(SourceArg::Flag {
                value: input.parse()?,
            })
        } else if lookahead.peek(kw::from) {
            let content;
            Ok(SourceArg::From {
                from_token: input.parse()?,
                paren_token: parenthesized!(content in input),
                r#type: content.parse()?,
                comma_token: content.parse()?,
                expr: content.parse()?,
            })
        } else {
            Err(lookahead.error())
        }
    }
}

impl ToTokens for SourceArg {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            SourceArg::Flag { value } => {
                value.to_tokens(tokens);
            }
            SourceArg::From {
                from_token,
                paren_token,
                r#type,
                comma_token,
                expr,
            } => {
                from_token.to_tokens(tokens);
                paren_token.surround(tokens, |tokens| {
                    r#type.to_tokens(tokens);
                    comma_token.to_tokens(tokens);
                    expr.to_tokens(tokens);
                })
            }
        }
    }
}

struct Visibility {
    visibility_token: kw::visibility,
    visibility: MaybeCompatArg<syn::Visibility>,
}

impl Visibility {
    // TODO: Remove boxed trait object
    fn into_arbitrary(self) -> Box<dyn ToTokens> {
        // TODO: Move this default value out of parsing
        self.visibility
            .into_option()
            .map_or_else(super::private_visibility, |v| Box::new(v))
    }
}

impl Parse for Visibility {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            visibility_token: input.parse()?,
            visibility: input.parse()?,
        })
    }
}

impl ToTokens for Visibility {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.visibility_token.to_tokens(tokens);
        self.visibility.to_tokens(tokens);
    }
}

enum MaybeArg<T> {
    None,
    Some {
        paren_token: token::Paren,
        content: T,
    },
}

impl<T> MaybeArg<T> {
    fn into_option(self) -> Option<T> {
        match self {
            MaybeArg::None => None,
            MaybeArg::Some { content, .. } => Some(content),
        }
    }

    fn parse_with<F>(input: ParseStream<'_>, parser: F) -> Result<Self>
    where
        F: FnOnce(ParseStream<'_>) -> Result<T>,
    {
        let lookahead = input.lookahead1();
        if lookahead.peek(token::Paren) {
            let content;
            Ok(MaybeArg::Some {
                paren_token: parenthesized!(content in input),
                content: parser(&content)?,
            })
        } else {
            Ok(MaybeArg::None)
        }
    }
}

impl<T: Parse> Parse for MaybeArg<T> {
    fn parse(input: ParseStream) -> Result<Self> {
        Self::parse_with(input, Parse::parse)
    }
}

impl<T: ToTokens> ToTokens for MaybeArg<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let MaybeArg::Some {
            paren_token,
            content,
        } = self
        {
            paren_token.surround(tokens, |tokens| {
                content.to_tokens(tokens);
            });
        }
    }
}

// TODO: Remove this with a semver-incompatible release
enum CompatArg<T> {
    Compat {
        eq_token: token::Eq,
        str: LitStr,
        content: T,
    },
    Pretty {
        paren_token: token::Paren,
        content: T,
    },
}

impl<T> CompatArg<T> {
    fn into_value(self) -> T {
        match self {
            CompatArg::Compat { content, .. } => content,
            CompatArg::Pretty { content, .. } => content,
        }
    }

    fn parse_with<F>(input: ParseStream<'_>, mut parser: F) -> Result<Self>
    where
        F: FnMut(ParseStream<'_>) -> Result<T>,
    {
        let lookahead = input.lookahead1();
        if lookahead.peek(token::Paren) {
            let content;
            Ok(CompatArg::Pretty {
                paren_token: parenthesized!(content in input),
                content: parser(&content)?,
            })
        } else if lookahead.peek(token::Eq) {
            let eq_token = input.parse()?;
            let str: LitStr = input.parse()?;

            let parser_with_parens = |input: &ParseBuffer| {
                let content;
                parenthesized!(content in input);
                parser(&content)
            };

            let content = str
                .parse_with(parser_with_parens)
                .or_else(|_| str.parse_with(parser))?;

            Ok(CompatArg::Compat {
                eq_token,
                str,
                content,
            })
        } else {
            Err(lookahead.error())
        }
    }
}

impl<T: Parse> Parse for CompatArg<T> {
    fn parse(input: ParseStream) -> Result<Self> {
        Self::parse_with(input, Parse::parse)
    }
}

impl<T: ToTokens> ToTokens for CompatArg<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            CompatArg::Compat { eq_token, str, .. } => {
                eq_token.to_tokens(tokens);
                str.to_tokens(tokens);
            }
            CompatArg::Pretty {
                paren_token,
                content,
            } => {
                paren_token.surround(tokens, |tokens| {
                    content.to_tokens(tokens);
                });
            }
        }
    }
}

// TODO: Remove this with a semver-incompatible release
enum MaybeCompatArg<T> {
    None,
    Compat {
        eq_token: token::Eq,
        str: LitStr,
        content: T,
    },
    Pretty {
        paren_token: token::Paren,
        content: T,
    },
}

impl<T> MaybeCompatArg<T> {
    fn into_option(self) -> Option<T> {
        match self {
            MaybeCompatArg::None => None,
            MaybeCompatArg::Compat { content, .. } => Some(content),
            MaybeCompatArg::Pretty { content, .. } => Some(content),
        }
    }

    fn parse_with<F>(input: ParseStream<'_>, parser: F) -> Result<Self>
    where
        F: FnOnce(ParseStream<'_>) -> Result<T>,
    {
        let lookahead = input.lookahead1();
        if lookahead.peek(token::Paren) {
            let content;
            Ok(MaybeCompatArg::Pretty {
                paren_token: parenthesized!(content in input),
                content: parser(&content)?,
            })
        } else if lookahead.peek(token::Eq) {
            let eq_token = input.parse()?;
            let str: LitStr = input.parse()?;
            let content = str.parse_with(parser)?;

            Ok(MaybeCompatArg::Compat {
                eq_token,
                str,
                content,
            })
        } else {
            Ok(MaybeCompatArg::None)
        }
    }
}

impl<T: Parse> Parse for MaybeCompatArg<T> {
    fn parse(input: ParseStream) -> Result<Self> {
        Self::parse_with(input, Parse::parse)
    }
}

impl<T: ToTokens> ToTokens for MaybeCompatArg<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            MaybeCompatArg::None => { /* no-op */ }
            MaybeCompatArg::Compat { eq_token, str, .. } => {
                eq_token.to_tokens(tokens);
                str.to_tokens(tokens);
            }
            MaybeCompatArg::Pretty {
                paren_token,
                content,
            } => {
                paren_token.surround(tokens, |tokens| {
                    content.to_tokens(tokens);
                });
            }
        }
    }
}
