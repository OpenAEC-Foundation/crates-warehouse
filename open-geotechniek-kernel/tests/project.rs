use open_geotechniek_kernel::{
    DuplicatePolicy, GeotechnicalProject, KernelError, ObjectKind, ProjectMetadata,
};

#[test]
fn rejects_duplicate_bro_ids() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    let xml = include_str!("fixtures/bhr-gt-minimal.xml");
    project.import_bro(xml, "first.xml").unwrap();
    let error = project.import_bro(xml, "second.xml").unwrap_err();
    assert!(matches!(error, KernelError::DuplicateObject { ref id } if id == "BHR000000000001"));
}

#[test]
fn object_order_is_deterministic() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    project
        .import_bro(include_str!("fixtures/bhr-g-minimal.xml"), "g.xml")
        .unwrap();
    project
        .import_bro(include_str!("fixtures/bhr-gt-minimal.xml"), "gt.xml")
        .unwrap();
    let ids: Vec<_> = project.objects().map(|object| object.id()).collect();
    assert_eq!(ids, vec!["BHR000000000001", "BHR000000000002"]);
}

#[test]
fn supports_crud_metadata_and_typed_iteration() {
    let metadata = ProjectMetadata {
        title: "Dijkvak".to_owned(),
        ..ProjectMetadata::default()
    };
    let mut project = GeotechnicalProject::new(metadata.clone());
    assert_eq!(project.metadata(), &metadata);

    let object = project
        .import_bro(include_str!("fixtures/cpt-minimal.xml"), "cpt.xml")
        .unwrap();
    assert_eq!(object.kind(), ObjectKind::Cpt);
    assert_eq!(project.cpts().count(), 1);
    assert_eq!(project.get("CPT000000000001").unwrap().id(), object.id());

    let replacement = ProjectMetadata {
        author: "Auteur".to_owned(),
        ..ProjectMetadata::default()
    };
    project.set_metadata(replacement.clone());
    assert_eq!(project.metadata(), &replacement);
    assert_eq!(project.remove(object.id()).unwrap().id(), object.id());
    assert!(matches!(
        project.get(object.id()),
        Err(KernelError::ObjectNotFound { ref id }) if id == object.id()
    ));
}

#[test]
fn merge_policy_is_explicit_and_atomic_when_rejecting_duplicates() {
    let mut target = GeotechnicalProject::new(ProjectMetadata::default());
    target
        .import_bro(include_str!("fixtures/bhr-gt-minimal.xml"), "target.xml")
        .unwrap();

    let mut incoming = GeotechnicalProject::new(ProjectMetadata::default());
    incoming
        .import_bro(include_str!("fixtures/bhr-gt-minimal.xml"), "duplicate.xml")
        .unwrap();
    incoming
        .import_bro(include_str!("fixtures/bhr-g-minimal.xml"), "new.xml")
        .unwrap();

    let error = target
        .merge_from(incoming, DuplicatePolicy::Reject)
        .unwrap_err();
    assert!(matches!(error, KernelError::DuplicateObject { ref id } if id == "BHR000000000001"));
    assert!(matches!(
        target.get("BHR000000000002"),
        Err(KernelError::ObjectNotFound { .. })
    ));
}

#[test]
fn replace_merge_overwrites_existing_objects() {
    let mut target = GeotechnicalProject::new(ProjectMetadata::default());
    target
        .import_bro(include_str!("fixtures/cpt-minimal.xml"), "old.xml")
        .unwrap();
    let mut incoming = GeotechnicalProject::new(ProjectMetadata::default());
    incoming
        .import_bro(include_str!("fixtures/cpt-minimal.xml"), "new.xml")
        .unwrap();

    target
        .merge_from(incoming, DuplicatePolicy::Replace)
        .unwrap();
    assert_eq!(
        target.cpts().next().unwrap().metadata.source_file,
        "new.xml"
    );
}

#[test]
fn detects_layers_only_for_cpt_objects() {
    let mut project = GeotechnicalProject::new(ProjectMetadata::default());
    project
        .import_bro(include_str!("fixtures/cpt-minimal.xml"), "cpt.xml")
        .unwrap();
    project
        .import_bro(include_str!("fixtures/bhr-gt-minimal.xml"), "bhr.xml")
        .unwrap();

    let layers = project.detect_cpt_layers("CPT000000000001").unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].depth_top, 1.0);
    assert!(matches!(
        project.detect_cpt_layers("BHR000000000001"),
        Err(KernelError::Conversion { .. })
    ));
}
