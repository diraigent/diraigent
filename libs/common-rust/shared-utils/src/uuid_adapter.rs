use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

pub mod as_uuid {
    use super::*;

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if bytes.is_empty() {
            serializer.serialize_str("")
        } else {
            let u = Uuid::from_slice(bytes).map_err(serde::ser::Error::custom)?;
            serializer.serialize_str(&u.to_string())
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.is_empty() {
            Ok(Vec::new())
        } else {
            let u = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
            Ok(u.as_bytes().to_vec())
        }
    }
}

pub mod vec_as_uuid {
    use super::*;

    pub fn serialize<S>(v: &[Vec<u8>], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let strs: Vec<String> = v
            .iter()
            .map(|b| {
                if b.is_empty() {
                    "".to_string()
                } else {
                    Uuid::from_slice(b)
                        .map(|u| u.to_string())
                        .unwrap_or_else(|_| "".to_string())
                }
            })
            .collect();
        strs.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let strs: Vec<String> = Vec::<String>::deserialize(deserializer)?;
        let mut out = Vec::with_capacity(strs.len());
        for s in strs {
            if s.is_empty() {
                out.push(Vec::new());
            } else {
                let u = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
                out.push(u.as_bytes().to_vec());
            }
        }
        Ok(out)
    }
}
