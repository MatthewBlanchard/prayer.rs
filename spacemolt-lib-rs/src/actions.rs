//! Generated SpaceMolt command metadata.

/// Whether a SpaceMolt action resolves synchronously or queues for a game tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// Query commands return a `result` frame synchronously.
    Query,
    /// Mutation commands first return a pending ack and later an action outcome.
    Mutation,
}

/// One generated request parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamDef {
    /// Parameter name as declared by the OpenAPI spec.
    pub name: &'static str,
    /// Human-readable schema type rendered from the OpenAPI property.
    pub ty: &'static str,
    /// Whether the request body requires this field.
    pub required: bool,
    /// Parameter description from the OpenAPI spec, when present.
    pub description: Option<&'static str>,
    /// Enum values from the OpenAPI property, when present.
    pub enum_values: &'static [&'static str],
    /// Positional index used by the server help/spec for CLI-like arguments.
    pub positional: Option<i64>,
}

/// One generated SpaceMolt action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionDef {
    /// Stable key in `tool/action` form.
    pub key: &'static str,
    /// OpenAPI tool name.
    pub tool: &'static str,
    /// OpenAPI action name.
    pub action: &'static str,
    /// HTTP v2 path for compatibility transports.
    pub path: &'static str,
    /// Query or mutation classification from `x-is-mutation`.
    pub kind: ActionKind,
    /// One-line summary from the OpenAPI operation, when present.
    pub summary: Option<&'static str>,
    /// Request parameters declared for the JSON body.
    pub params: &'static [ParamDef],
    /// Rust type name of the query's `structuredContent` response.
    pub response_type: Option<&'static str>,
    /// Rust type name of the mutation's `delta.details` response.
    pub details_type: Option<&'static str>,
}

include!(concat!(env!("OUT_DIR"), "/actions.gen.rs"));

/// Find an action by `tool/action` key.
pub fn find_action(key: &str) -> Option<&'static ActionDef> {
    ACTIONS.iter().find(|action| action.key == key)
}

/// Find an action by split tool and action names.
pub fn find_action_parts(tool: &str, action: &str) -> Option<&'static ActionDef> {
    ACTIONS
        .iter()
        .find(|def| def.tool == tool && def.action == action)
}

/// Returns true when the generated action is a mutation.
pub fn is_mutation(tool: &str, action: &str) -> Option<bool> {
    find_action_parts(tool, action).map(|def| def.kind == ActionKind::Mutation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_catalog_is_populated_and_keyed() {
        assert!(ACTIONS.len() > 200);
        for action in ACTIONS {
            assert_eq!(action.key, format!("{}/{}", action.tool, action.action));
            assert_eq!(action.path, format!("/api/v2/{}", action.key));
        }
    }

    #[test]
    fn known_actions_have_expected_kind() {
        assert_eq!(
            find_action("spacemolt/jump").map(|a| a.kind),
            Some(ActionKind::Mutation)
        );
        assert_eq!(
            find_action("spacemolt/mine").map(|a| a.kind),
            Some(ActionKind::Mutation)
        );
        assert_eq!(
            find_action("spacemolt/get_status").map(|a| a.kind),
            Some(ActionKind::Query)
        );
    }

    #[test]
    fn known_params_are_exposed() {
        let jump = find_action("spacemolt/jump").expect("jump action");
        assert!(jump.params.iter().any(|param| param.name == "id"));

        let buy = find_action("spacemolt/buy").expect("buy action");
        assert!(buy.params.iter().any(|param| param.name == "id"));
        assert!(buy.params.iter().any(|param| param.name == "quantity"));

        let create_buy_order =
            find_action("spacemolt_market/create_buy_order").expect("create_buy_order action");
        assert!(create_buy_order
            .params
            .iter()
            .any(|param| param.name == "item_id"));
    }

    #[test]
    fn bulk_array_object_params_keep_element_shapes() {
        fn param_ty(key: &str, name: &str) -> Option<&'static str> {
            find_action(key)?
                .params
                .iter()
                .find(|param| param.name == name)
                .map(|param| param.ty)
        }

        assert_eq!(
            param_ty("spacemolt_storage/deposit", "items"),
            Some("{ item_id: string; quantity: integer }[]")
        );
        assert_eq!(
            param_ty("spacemolt_market/create_sell_order", "orders"),
            Some("{ item_id: string; price_each: integer; quantity: integer }[]")
        );
        assert_eq!(
            param_ty("spacemolt_transfer/trade_offer", "offer_items"),
            Some("{ item_id?: string; quantity?: integer }[]")
        );
        assert_eq!(
            param_ty("spacemolt_market/create_buy_order", "orders"),
            Some(
                "{ deliver_to?: \"cargo\" | \"storage\"; item_id: string; price_each: integer; quantity: integer }[]"
            )
        );
        assert_eq!(
            param_ty("spacemolt/craft", "jobs"),
            Some("{ deliver_to?: string; facility_id?: string; items?: { item_id: string; quantity: integer }[]; label?: string; package_id?: string; preset?: string; quantity?: integer; recipe_id: string; source?: string; target?: string }[]")
        );
    }

    #[test]
    fn array_of_enum_params_parenthesize_union_element_type() {
        let get_notifications =
            find_action("spacemolt/get_notifications").expect("get_notifications");
        let types = get_notifications
            .params
            .iter()
            .find(|param| param.name == "types")
            .expect("types param");

        assert_eq!(
            types.ty,
            "(\"chat\" | \"combat\" | \"trade\" | \"market\" | \"crafting\" | \"system\")[]"
        );
    }

    #[test]
    fn latest_spec_version_is_exposed() {
        assert!(
            GENERATED_SPEC_VERSION.starts_with('v') && GENERATED_SPEC_VERSION.len() > 1,
            "generated SpaceMolt version should be populated"
        );
    }

    #[test]
    fn actions_expose_response_and_details_types() {
        let find_route = find_action("spacemolt/find_route").expect("find_route");
        assert_eq!(find_route.response_type, Some("FindRouteResponse"));
        assert_eq!(find_route.details_type, None);

        let jump = find_action("spacemolt/jump").expect("jump");
        assert_eq!(jump.response_type, None);
        assert_eq!(jump.details_type, Some("JumpCommandResponse"));

        for action in ACTIONS
            .iter()
            .filter(|action| action.details_type.is_some())
        {
            assert_eq!(action.kind, ActionKind::Mutation);
        }
    }
}
