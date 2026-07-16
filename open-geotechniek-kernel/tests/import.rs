use open_geotechniek_kernel::{
    GeotechnicalObject, GeotechnicalProject, KernelError, ProjectMetadata,
};

#[test]
fn imports_bro_cpt_into_existing_cpt_domain() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let object = project
        .import_bro(include_str!("fixtures/cpt-minimal.xml"), "cpt.xml")
        .unwrap();
    let GeotechnicalObject::Cpt(cpt) = object else {
        panic!("expected CPT")
    };
    assert_eq!(cpt.id, "CPT000000000001");
    assert_eq!(cpt.metadata.source_file, "cpt.xml");
    assert_eq!(
        cpt.metadata.extra.get("cone_type").map(String::as_str),
        Some("electrical")
    );
    assert_eq!(
        cpt.metadata.extra.get("position_crs").map(String::as_str),
        Some("EPSG:28992")
    );
    assert_eq!(cpt.points.len(), 2);
    assert_eq!(cpt.points[0].depth, 1.0);
    assert_eq!(cpt.points[0].depth_nap, None);
    assert_eq!(cpt.points[0].qc, Some(13.3));
    assert_eq!(cpt.points[0].fs, Some(118.18));
    assert_eq!(cpt.points[0].rf, Some(124.24));
    assert_eq!(cpt.points[0].u2, Some(122.22));
    assert_eq!(cpt.points[0].inclination, Some(115.15));
    assert_eq!(cpt.position.unwrap().x_rd, 155_000.0);
}

#[test]
fn imports_bhr_with_source_filename_in_common_extensions() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let object = project
        .import_bro(
            include_str!("fixtures/bhr-gt-minimal.xml"),
            "field-bore.xml",
        )
        .unwrap();
    let GeotechnicalObject::BhrGt(document) = object else {
        panic!("expected BHR-GT")
    };
    assert_eq!(
        document.common.extensions.get("openGeo/sourceFile"),
        Some(&"field-bore.xml".to_owned())
    );
    let GeotechnicalObject::BhrGt(stored) = project.get("BHR000000000001").unwrap() else {
        panic!("expected stored BHR-GT")
    };
    assert_eq!(
        stored.common.extensions.get("openGeo/sourceFile"),
        Some(&"field-bore.xml".to_owned())
    );
}

#[test]
fn preserves_non_rd_coordinates_without_typed_rd_position() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let object = project
        .import_bro(include_str!("fixtures/cpt-non-rd.xml"), "non-rd.xml")
        .unwrap();
    let GeotechnicalObject::Cpt(cpt) = object else {
        panic!("expected CPT")
    };

    assert_eq!(cpt.id, "CPT000000000003");
    assert!(cpt.position.is_none());
    assert_eq!(
        cpt.metadata.extra.get("position_crs").map(String::as_str),
        Some("EPSG:4326")
    );
    assert_eq!(
        cpt.metadata.extra.get("position_x").map(String::as_str),
        Some("5.1")
    );
    assert_eq!(
        cpt.metadata.extra.get("position_y").map(String::as_str),
        Some("52.1")
    );
    assert_eq!(project.objects().count(), 1);
    assert_eq!(project.cpts().count(), 1);
    assert_eq!(project.get("CPT000000000003").unwrap().id(), cpt.id);
}

#[test]
fn calculates_nap_depth_only_when_vertical_offset_exists() {
    let xml = include_str!("fixtures/cpt-minimal.xml").replace(
        "<conePenetrationTest>",
        "<deliveredVerticalPosition><offset>2.5</offset><verticalDatum>NAP</verticalDatum></deliveredVerticalPosition><conePenetrationTest>",
    );
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let GeotechnicalObject::Cpt(cpt) = project.import_bro(&xml, "vertical.xml").unwrap() else {
        panic!("expected CPT")
    };
    assert_eq!(cpt.metadata.ground_level_nap, Some(2.5));
    assert_eq!(cpt.points[0].depth_nap, Some(1.5));
    assert_eq!(cpt.position.unwrap().z_nap, Some(2.5));
}

#[test]
fn imports_cpt_content_and_rejects_duplicate_ids() {
    let content = r#"#GEFID= 1, 0, 0
#TESTID= CPT-GEF
#COLUMN= 2
#COLUMNINFO= 1, m, Length, 1
#COLUMNINFO= 2, MPa, Qc, 2
#EOH=
0.02 5.5
"#;
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let first = project.import_cpt(content, "first.gef").unwrap();
    assert_eq!(first.metadata.source_file, "first.gef");
    let error = project.import_cpt(content, "second.gef").unwrap_err();
    assert!(matches!(error, KernelError::DuplicateObject { ref id } if id == "CPT-GEF"));
}

#[test]
fn imports_ifcgeo_content_by_source_extension() {
    let content = r#"{
        "id": "CPT-JSON",
        "metadata": { "source_file": "old.ifcgeo" },
        "position": null,
        "points": []
    }"#;
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let cpt = project.import_cpt(content, "new.ifcgeo").unwrap();
    assert_eq!(cpt.id, "CPT-JSON");
    assert_eq!(cpt.metadata.source_file, "new.ifcgeo");
}

#[test]
fn recognizes_delivered_rd_urn_without_using_standardized_coordinates() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let object = project
        .import_bro(
            include_str!("fixtures/cpt-dispatch-location.xml"),
            "dispatch.xml",
        )
        .unwrap();
    let GeotechnicalObject::Cpt(cpt) = object else {
        panic!("expected CPT")
    };
    let position = cpt.position.unwrap();

    assert_eq!(position.x_rd, 155_123.4);
    assert_eq!(position.y_rd, 463_567.8);
    assert_eq!(
        cpt.metadata.extra.get("position_crs").map(String::as_str),
        Some("urn:ogc:def:crs:EPSG::28992")
    );
}

#[test]
fn recognizes_common_rd_authority_forms_without_substring_false_positives() {
    let fixture = include_str!("fixtures/cpt-dispatch-location.xml");
    for crs in [
        "EPSG:28992",
        "epsg/28992",
        "urn:ogc:def:crs:EPSG::28992",
        "https://www.opengis.net/def/crs/EPSG/0/28992",
    ] {
        let xml = fixture.replace("urn:ogc:def:crs:EPSG::28992", crs);
        let mut project = GeotechnicalProject::new(ProjectMetadata::default());
        let GeotechnicalObject::Cpt(cpt) = project.import_bro(&xml, "rd.xml").unwrap() else {
            panic!("expected CPT")
        };
        assert!(cpt.position.is_some(), "expected RD CRS: {crs}");
    }

    for crs in [
        "EPSG:289920",
        "NOT-EPSG:28992",
        "urn:ogc:def:crs:EPSG::4258",
    ] {
        let xml = fixture.replace("urn:ogc:def:crs:EPSG::28992", crs);
        let mut project = GeotechnicalProject::new(ProjectMetadata::default());
        let GeotechnicalObject::Cpt(cpt) = project.import_bro(&xml, "other.xml").unwrap() else {
            panic!("expected CPT")
        };
        assert!(cpt.position.is_none(), "unexpected RD CRS: {crs}");
    }
}
