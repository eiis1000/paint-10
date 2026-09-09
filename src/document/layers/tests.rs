use super::*;

fn solid(color: Color, width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_pixel(width, height, Rgba(color))
}

#[test]
fn stack_order_visibility_and_group_opacity_apply_once() {
    let mut document = Document::new(4, 4);
    document.add_layer().unwrap();
    document.image = solid([255, 0, 0, 255], 4, 4);
    document.add_object(Object::new(
        ObjectKind::Image(solid([0, 0, 255, 255], 2, 2)),
        (0, 0),
    ));
    document.active_layer_mut().opacity = 128;
    let pixels = document.composite();
    assert_eq!(pixels.get_pixel(0, 0).0, [127, 127, 255, 255]);
    assert_eq!(pixels.get_pixel(3, 3).0, [255, 127, 127, 255]);
    assert_eq!(
        document.active_composite().get_pixel(0, 0).0,
        [0, 0, 255, 255]
    );
    document.active_layer_mut().visible = false;
    assert_eq!(document.composite(), solid(WHITE, 4, 4));
    assert_eq!(
        document.active_composite().get_pixel(0, 0).0,
        [0, 0, 255, 255]
    );
    document.active_layer_mut().visible = true;
    document.move_layer(1, 0).unwrap();
    assert_eq!(document.composite(), solid(WHITE, 4, 4));
}

#[test]
fn raster_and_object_previews_respect_the_active_layer_depth() {
    let mut document = Document::new(4, 4);
    let red = Object::new(ObjectKind::Image(solid([255, 0, 0, 255], 4, 4)), (0, 0));
    let index = document.add_object(red);
    document.add_layer().unwrap();
    document.image.put_pixel(0, 0, Rgba([0, 255, 0, 255]));
    document.active_layer_mut().opacity = 128;
    document.set_active_layer(0).unwrap();
    let blue = Object::new(ObjectKind::Image(solid([0, 0, 255, 255], 4, 4)), (0, 0));
    let preview = document.composite_with_object(&blue, Some(index));
    assert_eq!(preview.get_pixel(0, 0).0, [0, 128, 127, 255]);
    assert_eq!(preview.get_pixel(1, 1).0, [0, 0, 255, 255]);
    document.objects[index] = blue.clone();
    assert_eq!(document.composite(), preview);

    document.image.put_pixel(3, 3, Rgba(BLACK));
    let appended = document.composite_with_object(&blue, None);
    document.add_object(blue);
    assert_eq!(document.composite(), appended);
    let raster = solid([255, 255, 0, 255], 4, 4);
    let preview = document.composite_with_raster(&raster, None);
    document.image = raster;
    assert_eq!(document.composite(), preview);
    assert_eq!(
        document.composite_without(Some(index)).get_pixel(0, 0).0,
        [127, 255, 0, 255]
    );
}

#[test]
fn editing_flattening_and_selection_do_not_change_other_layers() {
    let mut document = Document::new(6, 4);
    document.begin();
    document.add_layer().unwrap();
    document.add_object(Object::new(
        ObjectKind::Image(solid([200, 50, 20, 128], 2, 2)),
        (1, 1),
    ));
    document.commit();
    document.mark_saved();
    let background = document.layers()[0].clone();
    document.set_active_layer(0).unwrap();
    assert!(!document.dirty());
    document.set_active_layer(1).unwrap();
    let before = document.active_composite();
    document.begin();
    document.flatten();
    assert_eq!(document.active_composite(), before);
    assert!(document.layers()[0] == background);
    document.image.put_pixel(0, 0, Rgba([20, 80, 220, 255]));
    document.commit();
    assert!(document.dirty());
    document.undo();
    assert!(!document.dirty());
    assert_eq!(document.active_composite(), before);
    assert_eq!(document.objects.len(), 1);
    document.redo();
    assert!(document.objects.is_empty());
    assert_eq!(document.image.get_pixel(0, 0).0, [20, 80, 220, 255]);
}

#[test]
fn stack_operations_and_metadata_survive_undo_redo_and_cancel() {
    let mut document = Document::new(4, 3);
    document.begin();
    let red = document.add_layer().unwrap();
    document.image = solid([255, 0, 0, 255], 4, 3);
    document.active_layer_mut().name = "Ink".into();
    document.commit();
    document.begin();
    let copy = document.duplicate_layer(red).unwrap();
    document.active_layer_mut().opacity = 100;
    document.move_layer(copy, 0).unwrap();
    document.commit();
    assert_eq!(document.active_layer_index(), 0);
    assert_eq!(document.layers()[0].name, "Ink copy");
    document.undo();
    assert_eq!(document.layer_count(), 2);
    assert_eq!(document.active_layer_index(), red);
    document.redo();
    assert_eq!(document.layers()[0].opacity, 100);
    document.begin();
    document.delete_layer(0).unwrap();
    document.cancel();
    assert_eq!(document.layer_count(), 3);
    assert_eq!(document.active_layer_index(), 0);
    document.begin();
    document.delete_layer(0).unwrap();
    document.commit();
    document.undo();
    assert_eq!(document.layer_count(), 3);
}

#[test]
fn full_opacity_merge_preserves_editable_objects_and_translucent_merge_preserves_pixels() {
    for opacity in [255, 128] {
        let mut document = Document::from_image(solid([220, 160, 80, 128], 5, 4));
        document.add_layer().unwrap();
        document.add_object(Object::new(
            ObjectKind::Image(solid([30, 70, 220, 190], 3, 2)),
            (1, 1),
        ));
        document.active_layer_mut().opacity = opacity;
        let before = document.composite();
        document.begin();
        document.merge_layer_down(1).unwrap();
        document.commit();
        assert_eq!(document.layer_count(), 1);
        assert_eq!(document.composite(), before);
        if opacity == 255 {
            assert!(document
                .objects
                .iter()
                .any(|object| matches!(object.kind, ObjectKind::Image(_))));
        }
        document.undo();
        assert_eq!(document.layer_count(), 2);
        assert_eq!(document.active_layer().opacity, opacity);
        assert_eq!(document.composite(), before);
        document.redo();
        assert_eq!(document.composite(), before);
    }
}

#[test]
fn locked_layers_cannot_be_deleted_or_merged_and_last_layer_is_protected() {
    let mut document = Document::new(2, 2);
    assert!(document.delete_layer(0).is_err());
    assert!(document.merge_layer_down(0).is_err());
    assert!(document.set_active_layer(2).is_err());
    document.add_layer().unwrap();
    document.layer_mut(0).unwrap().locked = true;
    assert!(document.delete_layer(0).is_err());
    assert!(document.merge_layer_down(1).is_err());
    document.active_layer_mut().visible = false;
    document.layer_mut(0).unwrap().locked = false;
    assert!(document.merge_layer_down(1).is_err());
}

#[test]
fn canvas_transforms_cover_hidden_layers_and_preserve_original_sources() {
    let source = RgbaImage::from_fn(12, 8, |x, y| Rgba([x as u8 * 15, y as u8 * 20, 80, 255]));
    let mut document = Document::from_image(source.clone());
    document.add_layer().unwrap();
    document.add_object(Object::new(ObjectKind::Image(source.clone()), (0, 0)));
    document.active_layer_mut().visible = false;
    document.active_layer_mut().locked = true;
    document
        .resize_content(3, 2, ImageSampling::Smooth)
        .unwrap();
    document
        .resize_content(12, 8, ImageSampling::Smooth)
        .unwrap();
    for layer in document.layers() {
        assert_eq!(layer.image.dimensions(), (12, 8));
        let object = &layer.objects[0];
        match &object.kind {
            ObjectKind::Image(image) | ObjectKind::Raster(image) => assert_eq!(image, &source),
            _ => panic!("Original source remains a raster or image"),
        }
        assert_eq!(layer.composite(), source);
    }
    let crop = Region {
        x: 2,
        y: 1,
        w: 6,
        h: 4,
    };
    document.crop_canvas(crop).unwrap();
    for layer in document.layers() {
        assert_eq!(layer.composite(), crop.extract(&source));
    }
    document.rotate_content(90.0, WHITE).unwrap();
    for layer in document.layers() {
        assert_eq!(layer.image.dimensions(), (4, 6));
        assert_eq!(layer.objects[0].source_dimensions(), source.dimensions());
    }
    document.flip_content(true);
    document.flip_content(true);
    document.skew_content(20.0, 0.0, WHITE).unwrap();
    assert_eq!(
        document.layers()[0].image.dimensions(),
        document.layers()[1].image.dimensions()
    );
    assert!(!document.layers()[1].visible);
    assert!(document.layers()[1].locked);
    assert_eq!(
        document.layers()[1].objects[0].source_dimensions(),
        source.dimensions()
    );
}

#[test]
fn paper_identity_survives_reordering_and_transparent_layers_expand_transparently() {
    let mut document = Document::new(3, 2);
    document.active_layer_mut().name = "Paper".into();
    document.add_layer().unwrap();
    document.active_layer_mut().name = "Background".into();
    document.move_layer(0, 1).unwrap();
    document.resize_canvas(5, 4, [220, 210, 190, 255]).unwrap();
    assert_eq!(document.layers()[0].composite().get_pixel(4, 3)[3], 0);
    assert_eq!(
        document.layers()[1].composite().get_pixel(4, 3).0,
        [220, 210, 190, 255]
    );
    let index = document.duplicate_layer(1).unwrap();
    assert!(!document.layers()[index].is_background);
}

#[test]
fn thumbnail_is_bounded_and_preserves_retained_depth_and_source() {
    let source = solid([220, 40, 30, 255], 1000, 800);
    let mut document = Document::from_image(RgbaImage::new(1000, 800));
    document.add_object(Object::new(ObjectKind::Image(source.clone()), (0, 0)));
    document.image.put_pixel(500, 400, Rgba([0, 0, 255, 255]));
    document.active_layer_mut().opacity = 20;
    document.active_layer_mut().visible = false;
    let thumbnail = document.active_layer().thumbnail(50);
    assert_eq!(thumbnail.dimensions(), (50, 40));
    assert_eq!(thumbnail.get_pixel(0, 0).0, [220, 40, 30, 255]);
    let ObjectKind::Image(original) = &document.objects[0].kind else {
        panic!()
    };
    assert_eq!(original, &source);
}

#[test]
fn invalid_whole_picture_transform_is_atomic_and_layer_count_is_bounded() {
    let mut document = Document::new(2, 2);
    for _ in 1..MAX_LAYERS {
        document.add_layer().unwrap();
    }
    assert!(document.add_layer().is_err());
    assert!(document.duplicate_layer(0).is_err());
    let original = document.layers.clone();
    assert!(document
        .resize_content(4096, 4096, ImageSampling::Smooth)
        .is_err());
    assert!(document.skew_content(45.0, 45.0, WHITE).is_err());
    assert!(document.layers == original);
}

#[test]
fn structural_operations_cannot_cross_the_project_wide_object_limit() {
    let object = Object::new(ObjectKind::Image(RgbaImage::new(1, 1)), (0, 0));
    let mut document = Document::new(4, 3);
    document.objects = vec![object.clone(); 600];
    let before = document.layers.clone();
    assert!(document
        .duplicate_layer(0)
        .unwrap_err()
        .contains("1,000 object"));
    assert!(document.layers == before);
    assert_eq!(document.active_layer_index(), 0);

    document.objects.resize(500, object.clone());
    document.add_layer().unwrap();
    document.objects = vec![object.clone(); 500];
    let before = document.layers.clone();
    // Merging the lower layer's painted raster into its object stack adds one
    // object. Reject before replacing/removing either source layer.
    assert!(document
        .merge_layer_down(1)
        .unwrap_err()
        .contains("1,000 object"));
    assert!(document.layers == before);
    assert_eq!(document.active_layer_index(), 1);

    let mut document = Document::new(4, 3);
    document.objects = vec![object; MAX_OBJECTS];
    let before = document.layers.clone();
    type Transform = fn(&mut Document) -> Result<(), String>;
    let operations: [Transform; 5] = [
        |document| document.resize_content(2, 2, ImageSampling::Smooth),
        |document| document.resize_canvas(8, 6, WHITE),
        |document| {
            document.crop_canvas(Region {
                x: 0,
                y: 0,
                w: 2,
                h: 2,
            })
        },
        |document| document.rotate_content(90.0, WHITE),
        |document| document.skew_content(20.0, 0.0, WHITE),
    ];
    for operation in operations {
        assert!(operation(&mut document)
            .unwrap_err()
            .contains("1,000 object"));
        assert!(document.layers == before);
        assert!(!document.dirty());
    }
    assert!(crate::project::encode(&document).is_ok());
}

#[test]
fn duplication_checks_object_asset_bytes_before_cloning_sources() {
    let mut document = Document::new(2, 2);
    // Duplicating 64 MiB plus one pixel remains below the total layer budget,
    // but crosses the independent 128 MiB object-asset budget.
    document.objects = vec![
        Object::new(ObjectKind::Image(RgbaImage::new(4096, 4096)), (0, 0)),
        Object::new(ObjectKind::Image(RgbaImage::new(1, 1)), (0, 0)),
    ];
    validate_layers(document.layers(), 0).unwrap();
    assert!(document.duplicate_layer(0).unwrap_err().contains("128 MB"));
    assert_eq!(document.layer_count(), 1);
    assert_eq!(document.objects.len(), 2);
    assert_eq!(document.objects[0].source_dimensions(), (4096, 4096));
    assert!(!document.dirty());
}
