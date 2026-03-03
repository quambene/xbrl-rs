use crate::{
    TaxonomySchema, XbrlError,
    taxonomy::schema::{XbrlBase, is_absolute_uri, is_ncname, local_name},
};

/// Validate schema-level XBRL constraints.
pub fn validate(taxonomy: &TaxonomySchema) -> Result<(), XbrlError> {
    if taxonomy
        .target_namespace
        .as_deref()
        .is_some_and(|ns| ns.trim().is_empty())
    {
        return Err(XbrlError::InvalidSchemaDocument {
            path: taxonomy.file_path.clone(),
            reason: "empty targetNamespace is not allowed".to_string(),
        });
    }

    for element in &taxonomy.concepts {
        if element.substitution_group.is_item() && element.period_type.is_none() {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: format!("item '{}' is missing xbrli:periodType", element.qname.local),
            });
        }

        if element.substitution_group.is_tuple() && element.period_type.is_some() {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: format!(
                    "tuple '{}' must not declare xbrli:periodType",
                    element.qname.local
                ),
            });
        }

        if element.balance.is_some() {
            if element.substitution_group.is_tuple() {
                return Err(XbrlError::InvalidSchemaDocument {
                    path: taxonomy.file_path.clone(),
                    reason: format!(
                        "tuple '{}' must not declare xbrli:balance",
                        element.qname.local
                    ),
                });
            }

            let is_monetary = taxonomy.is_monetary_type(element.data_type.name.local.as_str())
                || matches!(element.data_type.base, XbrlBase::Monetary);

            if !is_monetary {
                return Err(XbrlError::InvalidSchemaDocument {
                    path: taxonomy.file_path.clone(),
                    reason: format!(
                        "element '{}' has xbrli:balance but is not monetaryItemType-derived",
                        element.qname.local
                    ),
                });
            }
        }

        if element.substitution_group.is_item()
            && taxonomy.is_known_complex_item_type(element.data_type.name.local.as_str())
        {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: format!(
                    "item '{}' has unsupported complex content type",
                    element.qname.local
                ),
            });
        }
    }

    for role_type in &taxonomy.role_types {
        if !role_type.id.is_empty() && !is_ncname(&role_type.id) {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: format!("roleType id '{}' is not an NCName", role_type.id),
            });
        }

        if role_type.role_uri.trim().is_empty() {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: "roleType roleURI is required".to_string(),
            });
        }
    }

    for arcrole_type in &taxonomy.arcrole_types {
        if !arcrole_type.id.is_empty() && !is_ncname(&arcrole_type.id) {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: format!("arcroleType id '{}' is not an NCName", arcrole_type.id),
            });
        }

        if arcrole_type.arcrole_uri.trim().is_empty() {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: "arcroleType arcroleURI is required".to_string(),
            });
        }

        if !is_absolute_uri(&arcrole_type.arcrole_uri) {
            return Err(XbrlError::InvalidSchemaDocument {
                path: taxonomy.file_path.clone(),
                reason: format!(
                    "arcroleType arcroleURI '{}' is not an absolute URI",
                    arcrole_type.arcrole_uri
                ),
            });
        }
    }

    if taxonomy.linkbase_refs.iter().any(|linkbase_ref| {
        linkbase_ref
            .role
            .as_deref()
            .is_some_and(|role| role.ends_with("/role/labelLinkbaseRef"))
    }) {
        for role_type in &taxonomy.role_types {
            if role_type
                .used_on
                .iter()
                .any(|value| local_name(value) == "label")
            {
                return Err(XbrlError::InvalidSchemaDocument {
                    path: taxonomy.file_path.clone(),
                    reason: "roleType usedOn label is not valid for standard label linkbase usage"
                        .to_string(),
                });
            }
        }
    }

    Ok(())
}
