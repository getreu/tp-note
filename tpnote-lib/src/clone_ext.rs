use std::borrow::Cow;

pub trait CloneExt<'b> {
    /// Clone a `Cow` without memory allocation.
    /// Note, the original must outlive the clone! Use case:
    /// ```no_run
    /// use crate::tpnote_lib::clone_ext::CloneExt;
    /// use std::borrow::Cow;
    /// fn do_something_or_nothing(v: Cow<str>) -> Cow<str> {
    ///     if v.len() > 3 {
    ///         let s = "Hello ".to_string() + &*v;
    ///         Cow::Owned(s)
    ///     } else {
    ///         v
    ///     }
    /// }
    ///
    /// // Sometimes, we only have a `&Cow`, but we need a `Cow`!
    /// let a: &Cow<str> = &Cow::Owned("world!".to_string());
    /// let b: Cow<str>  = a.shallow_clone();
    /// assert_eq!(do_something_or_nothing(b), "Hello world!");
    ///
    /// let a: &Cow<str> = &Cow::Owned("ld!".to_string());
    /// let b: Cow<str>  = a.shallow_clone();
    /// assert_eq!(do_something_or_nothing(b), "ld!");
    /// ```
    fn shallow_clone(&'b self) -> Cow<'b, str>;
}

impl<'b> CloneExt<'b> for Cow<'b, str> {
    fn shallow_clone(&'b self) -> Cow<'b, str> {
        // match *self {
        //     Self::Borrowed(b) => Self::Borrowed(b),
        //     Self::Owned(ref o) => Self::Borrowed(o.as_ref()),
        // }
        // // This is equivalent to:
        Cow::Borrowed(&**self)
    }
}
