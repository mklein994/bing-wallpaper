pub mod datetime {
    use jiff::{civil::DateTime, tz::TimeZone, Zoned};
    use serde::{de, ser};

    const FORMAT: &str = "%Y%m%d%H%M";

    struct Visitor;

    pub fn deserialize<'de, D: de::Deserializer<'de>>(d: D) -> Result<Zoned, D::Error> {
        d.deserialize_string(Visitor)
    }

    impl de::Visitor<'_> for Visitor {
        type Value = Zoned;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(&format!("a string formatted as {FORMAT}"))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::strptime(FORMAT, v)
                .and_then(|x| x.to_zoned(TimeZone::system()))
                .map_err(de::Error::custom)
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&v)
        }
    }

    pub fn serialize<S: ser::Serializer>(value: &Zoned, serializer: S) -> Result<S::Ok, S::Error> {
        super::serialize(value, serializer, FORMAT)
    }
}

pub mod date {
    use jiff::{civil::Date, tz::TimeZone, Zoned};
    use serde::{de, ser};

    const FORMAT: &str = "%Y%m%d";

    struct Visitor;

    pub fn deserialize<'de, D: de::Deserializer<'de>>(d: D) -> Result<Zoned, D::Error> {
        d.deserialize_string(Visitor)
    }

    impl de::Visitor<'_> for Visitor {
        type Value = Zoned;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(&format!("a string formatted as {FORMAT}"))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Date::strptime(FORMAT, v)
                .and_then(|x| x.to_zoned(TimeZone::system()))
                .map_err(de::Error::custom)
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&v)
        }
    }

    pub fn serialize<S: ser::Serializer>(value: &Zoned, serializer: S) -> Result<S::Ok, S::Error> {
        super::serialize(value, serializer, FORMAT)
    }
}

fn serialize<S: serde::Serializer>(
    value: &jiff::Zoned,
    serializer: S,
    format: &'static str,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(
        &jiff::fmt::strtime::format(format, value).map_err(serde::ser::Error::custom)?,
    )
}
