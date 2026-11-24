//! Checks on the native package definitions that need no packaging tools, so they run on every
//! platform with the workspace tests (the Windows Installer checks themselves need Windows:
//! `packaging/windows/check-install-policy.ps1`).

#[cfg(test)]
mod tests {
    /// The Windows Installer identity CADCraft's packages use (`packaging/windows/cadcraft.wxs`
    /// upstream): its upgrade code and component GUIDs. DraftCraft 1.0.0–2.2.2 shipped with the
    /// same ones, so each installer removed the other app or refused to install (#35).
    const CADCRAFT_IDS: [&str; 3] =
        ["119D7D76-0167-4A9E-8F37-09DA0235F68E", "2AC18056-48DE-4079-AB79-4FDEF4FBB075", "76029CD7-EBDB-420F-AC34-FF0E82A99E32"];

    fn read(path: &str) -> String {
        std::fs::read_to_string(crate::root().join(path)).unwrap()
    }

    /// The value of `attr="…"` in the first element of `xml` that starts with `element`.
    fn attr<'a>(xml: &'a str, element: &str, attr: &str) -> &'a str {
        let start = xml.find(element).unwrap_or_else(|| panic!("no {element}"));
        let tag = &xml[start..start + xml[start..].find('>').unwrap()];
        let at = tag.find(&format!(" {attr}=\"")).unwrap_or_else(|| panic!("{element} has no {attr}")) + attr.len() + 3;
        &tag[at..at + tag[at..].find('"').unwrap()]
    }

    fn version(v: &str) -> (u64, u64, u64) {
        let n: Vec<u64> = v.split('.').map(|p| p.parse().unwrap()).collect();
        (n[0], n[1], n[2])
    }

    #[test]
    fn the_windows_installer_has_its_own_identity_and_leaves_cadcraft_alone() {
        let wxs = read("packaging/windows/draftcraft.wxs");
        let policy: serde_json::Value = serde_json::from_str(&read("packaging/windows/install-policy.json")).unwrap();
        // Its own upgrade code, the one the policy checker expects.
        let code = attr(&wxs, "<Package ", "UpgradeCode");
        assert_eq!(policy["upgradeCode"], code);
        // No upgrade code or component GUID is CADCraft's, except the one Upgrade row below.
        let package_and_components: String = wxs.lines().filter(|l| !l.contains("<Upgrade Id=")).collect();
        for id in CADCRAFT_IDS {
            assert!(!package_and_components.to_ascii_uppercase().contains(id), "{id} is CADCraft's");
        }
        // Earlier DraftCraft versions, which had CADCraft's code, are still replaced: by version,
        // from 1.0.0 (the first release; CADCraft's are 0.x) to 2.2.2 (the last with that code).
        let legacy = &policy["legacyUpgrade"];
        assert_eq!(legacy["code"], CADCRAFT_IDS[0]);
        assert_eq!(attr(&wxs, "<Upgrade ", "Id"), CADCRAFT_IDS[0]);
        let row = &wxs[wxs.find("<Upgrade ").unwrap()..];
        for (name, value) in [("Minimum", &legacy["minimum"]), ("Maximum", &legacy["maximum"])] {
            assert_eq!(attr(row, "<UpgradeVersion ", name), value.as_str().unwrap(), "{name}");
        }
        assert_eq!(attr(row, "<UpgradeVersion ", "IncludeMinimum"), "yes");
        assert_eq!(attr(row, "<UpgradeVersion ", "IncludeMaximum"), "yes");
        let (min, max) = (version(attr(row, "<UpgradeVersion ", "Minimum")), version(attr(row, "<UpgradeVersion ", "Maximum")));
        assert!(min >= (1, 0, 0), "CADCraft releases are 0.x and must never match");
        assert_eq!(max, (2, 2, 2));
    }
}
