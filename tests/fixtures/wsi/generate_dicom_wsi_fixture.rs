//! Reproducible generator for the synthetic DICOM VL WSI JPEG2000 fixture.

use std::{env, fs, path::Path};

use dicom_core::{
    value::{fragments::Fragments, PixelFragmentSequence, Value},
    DataElement, PrimitiveValue, Tag, VR,
};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};

const VL_WSI_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";
const JPEG2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
const INSTANCE_UID: &str = "1.2.826.0.1.3680043.10.54321.1";
const SERIES_UID: &str = "1.2.826.0.1.3680043.10.54321";

fn main() {
    let mut args = env::args_os().skip(1);
    let input = args.next().expect("input J2K codestream path");
    let output = args.next().expect("output DICOM path");
    assert!(args.next().is_none(), "expected exactly two paths");
    let frame = fs::read(input).expect("read J2K codestream");

    let mut object = InMemDicomObject::new_empty();
    put_text(&mut object, Tag(0x0008, 0x0016), VR::UI, VL_WSI_STORAGE);
    put_text(&mut object, Tag(0x0008, 0x0018), VR::UI, INSTANCE_UID);
    put_text(&mut object, Tag(0x0020, 0x000e), VR::UI, SERIES_UID);
    put_text(
        &mut object,
        Tag(0x0008, 0x0008),
        VR::CS,
        "ORIGINAL\\PRIMARY\\VOLUME\\NONE",
    );
    put_u16(&mut object, Tag(0x0028, 0x0010), 12);
    put_u16(&mut object, Tag(0x0028, 0x0011), 16);
    put_u32(&mut object, Tag(0x0048, 0x0007), 12);
    put_u32(&mut object, Tag(0x0048, 0x0006), 16);
    put_text(&mut object, Tag(0x0028, 0x0008), VR::IS, "1");
    put_u16(&mut object, Tag(0x0028, 0x0002), 3);
    put_text(&mut object, Tag(0x0028, 0x0004), VR::CS, "RGB");
    put_u16(&mut object, Tag(0x0028, 0x0006), 0);
    put_u16(&mut object, Tag(0x0028, 0x0100), 8);
    put_u16(&mut object, Tag(0x0028, 0x0101), 8);
    put_u16(&mut object, Tag(0x0028, 0x0102), 7);
    put_u16(&mut object, Tag(0x0028, 0x0103), 0);
    put_text(&mut object, Tag(0x0028, 0x0030), VR::DS, "0.00025\\0.00025");

    let sequence = PixelFragmentSequence::from(vec![Fragments::new(frame, 0)]);
    object.put(DataElement::<InMemDicomObject>::new(
        Tag(0x7fe0, 0x0010),
        VR::OB,
        Value::from(sequence),
    ));

    let output = Path::new(&output);
    fs::create_dir_all(output.parent().expect("output has parent"))
        .expect("create output directory");
    object
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid(VL_WSI_STORAGE)
                .media_storage_sop_instance_uid(INSTANCE_UID)
                .transfer_syntax(JPEG2000_LOSSLESS),
        )
        .expect("build DICOM file metadata")
        .write_to_file(output)
        .expect("write DICOM fixture");
}

fn put_text(object: &mut InMemDicomObject, tag: Tag, vr: VR, value: &'static str) {
    object.put(DataElement::new(tag, vr, value));
}

fn put_u16(object: &mut InMemDicomObject, tag: Tag, value: u16) {
    object.put(DataElement::new(tag, VR::US, PrimitiveValue::from(value)));
}

fn put_u32(object: &mut InMemDicomObject, tag: Tag, value: u32) {
    object.put(DataElement::new(tag, VR::UL, PrimitiveValue::from(value)));
}
