pub struct ProtoDecoder<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ProtoDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn read_varint(&mut self) -> Option<u64> {
        let mut result = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = *self.data.get(self.pos)?;
            self.pos += 1;
            result |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 64 {
                return None;
            }
        }
        Some(result)
    }

    pub fn read_field(&mut self) -> Option<(u32, u8, ProtoValue<'a>)> {
        let tag = self.read_varint()?;
        let field_num = (tag >> 3) as u32;
        let wire_type = (tag & 0x7) as u8;

        let value = match wire_type {
            0 => ProtoValue::Varint(self.read_varint()?),
            1 => {
                if self.pos + 8 > self.data.len() {
                    return None;
                }
                let bytes = &self.data[self.pos..self.pos + 8];
                self.pos += 8;
                ProtoValue::Fixed64(bytes)
            }
            2 => {
                let len = self.read_varint()? as usize;
                if self.pos + len > self.data.len() {
                    return None;
                }
                let bytes = &self.data[self.pos..self.pos + len];
                self.pos += len;
                ProtoValue::LengthDelimited(bytes)
            }
            5 => {
                if self.pos + 4 > self.data.len() {
                    return None;
                }
                let bytes = &self.data[self.pos..self.pos + 4];
                self.pos += 4;
                ProtoValue::Fixed32(bytes)
            }
            _ => return None,
        };

        Some((field_num, wire_type, value))
    }

    pub fn read_all(&mut self) -> Vec<(u32, u8, ProtoValue<'a>)> {
        let mut fields = Vec::new();
        while self.pos < self.data.len() {
            if let Some(field) = self.read_field() {
                fields.push(field);
            } else {
                break;
            }
        }
        fields
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProtoValue<'a> {
    Varint(u64),
    Fixed64(&'a [u8]),
    LengthDelimited(&'a [u8]),
    Fixed32(&'a [u8]),
}

impl<'a> ProtoValue<'a> {
    pub fn as_bytes(&self) -> Option<&'a [u8]> {
        match self {
            ProtoValue::LengthDelimited(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&'a str> {
        match self {
            ProtoValue::LengthDelimited(bytes) => std::str::from_utf8(bytes).ok(),
            _ => None,
        }
    }

    pub fn as_varint(&self) -> Option<u64> {
        match self {
            ProtoValue::Varint(v) => Some(*v),
            _ => None,
        }
    }
}

pub fn find_field<'a>(fields: &'a [(u32, u8, ProtoValue<'a>)], num: u32) -> Option<&'a ProtoValue<'a>> {
    fields.iter().find(|(n, _, _)| *n == num).map(|(_, _, v)| v)
}

pub fn find_string(fields: &[(u32, u8, ProtoValue)], num: u32) -> String {
    find_field(fields, num)
        .and_then(|v| v.as_string())
        .unwrap_or("")
        .to_string()
}

pub fn find_varint(fields: &[(u32, u8, ProtoValue)], num: u32) -> Option<u64> {
    find_field(fields, num).and_then(|v| v.as_varint())
}

pub fn find_all_bytes<'a>(fields: &'a [(u32, u8, ProtoValue<'a>)], num: u32) -> Vec<&'a [u8]> {
    fields
        .iter()
        .filter(|(n, _, _)| *n == num)
        .filter_map(|(_, _, v)| v.as_bytes())
        .collect()
}

pub fn navigate(data: &[u8], path: &[u32]) -> Vec<u8> {
    let mut current = data;
    for &field_num in path {
        let mut decoder = ProtoDecoder::new(current);
        let fields = decoder.read_all();
        let Some(value) = fields.iter().find(|(n, _, _)| *n == field_num) else {
            return Vec::new();
        };
        match &value.2 {
            ProtoValue::LengthDelimited(bytes) => current = bytes,
            _ => return Vec::new(),
        }
    }
    current.to_vec()
}
