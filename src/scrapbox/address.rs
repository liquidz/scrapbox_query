use errors::*;
use scrapbox::tantivy::DocAddress;

/// Convert DocAddress to String
pub fn doc_address_to_string(addr: DocAddress) -> String {
    format!("{}:{}", addr.segment_ord(), addr.doc())
}

/// Convert String to DocAddress
pub fn string_to_doc_address(s: &str) -> Result<DocAddress> {
    let mut iter = s.split(':');
    let x = iter.next().ok_or("fixme")?;
    let y = iter.next().ok_or("fixme")?;
    let ux = x.parse::<u32>().chain_err(|| "failed to parse u32")?;
    let uy = y.parse::<u32>().chain_err(|| "failed to parse u32")?;
    Ok(DocAddress(ux, uy))
}

#[test]
fn test_convert_doc_address() {
    let s = "1:2";
    let addr = string_to_doc_address(s).unwrap();
    assert_eq!(doc_address_to_string(addr), s);
}
