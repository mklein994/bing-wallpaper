pub mod datetime {

    use jiff::{civil::DateTime, tz::TimeZone, Zoned};
    use serde::{de, ser};

    const FORMAT: &str = "%Y%m%d%H%M";

    struct Visitor;

    pub fn deserialize<'de, D: de::Deserializer<'de>>(d: D) -> Result<Zoned, D::Error> {
        d.deserialize_string(Visitor)
    }

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = Zoned;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(&format!("a string formatted as {FORMAT}"))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&v)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::strptime(FORMAT, v)
                .and_then(|x| x.to_zoned(TimeZone::system()))
                .map_err(de::Error::custom)
        }
    }

    pub fn serialize<S: ser::Serializer>(value: &Zoned, serializer: S) -> Result<S::Ok, S::Error> {
        serializer
            .serialize_str(&jiff::fmt::strtime::format(FORMAT, value).map_err(ser::Error::custom)?)
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

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = Zoned;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str(&format!("a string formatted as {FORMAT}"))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&v)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Date::strptime(FORMAT, v)
                .and_then(|x| x.to_zoned(TimeZone::system()))
                .map_err(de::Error::custom)
        }
    }

    pub fn serialize<S: ser::Serializer>(value: &Zoned, serializer: S) -> Result<S::Ok, S::Error> {
        serializer
            .serialize_str(&jiff::fmt::strtime::format(FORMAT, value).map_err(ser::Error::custom)?)
    }
}
