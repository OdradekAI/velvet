use std::fmt;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetId(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentKindId(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentFingerprint(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRef {
    pub id: AssetId,
    pub content_kind: ContentKindId,
    pub fingerprint: Option<ContentFingerprint>,
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
