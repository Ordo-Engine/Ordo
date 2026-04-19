use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::Response,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    ZhCn,
    ZhTw,
}

tokio::task_local! {
    static REQUEST_LOCALE: Locale;
}

pub async fn with_request_locale(req: Request<Body>, next: Next) -> Response {
    let locale = parse_accept_language(req.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()));
    REQUEST_LOCALE.scope(locale, next.run(req)).await
}

pub fn current_locale() -> Locale {
    REQUEST_LOCALE.try_with(|locale| *locale).unwrap_or(Locale::En)
}

pub fn localize_error(status: StatusCode, message: &str) -> String {
    let locale = current_locale();
    let trimmed = message.trim();
    translate_known_error(status, trimmed, locale).unwrap_or_else(|| trimmed.to_string())
}

pub fn localize_auth_missing_header() -> String {
    match current_locale() {
        Locale::En => "Missing Authorization header".to_string(),
        Locale::ZhCn => "缺少 Authorization 请求头".to_string(),
        Locale::ZhTw => "缺少 Authorization 請求標頭".to_string(),
    }
}

pub fn localize_permission_required(permission: &str) -> String {
    match current_locale() {
        Locale::En => format!("Permission '{}' required", permission),
        Locale::ZhCn => format!("需要权限“{}”", permission),
        Locale::ZhTw => format!("需要權限「{}」", permission),
    }
}

pub fn localize_invalid_token(details: &str) -> String {
    match current_locale() {
        Locale::En => format!("Invalid token: {}", details),
        Locale::ZhCn => format!("无效的令牌：{}", details),
        Locale::ZhTw => format!("無效的權杖：{}", details),
    }
}

pub fn localize_approver_not_member(user_id: &str) -> String {
    match current_locale() {
        Locale::En => format!("Approver '{}' is not a member of this organization", user_id),
        Locale::ZhCn => format!("审批人“{}”不是该组织成员", user_id),
        Locale::ZhTw => format!("審批人「{}」不是該組織成員", user_id),
    }
}

pub fn localize_requires_role_or_higher(role: &str) -> String {
    match current_locale() {
        Locale::En => format!("Requires {} role or higher", role),
        Locale::ZhCn => format!("需要 {} 或更高角色权限", role),
        Locale::ZhTw => format!("需要 {} 或更高角色權限", role),
    }
}

pub fn localize_invalid_github_base64(details: &str) -> String {
    match current_locale() {
        Locale::En => format!("Invalid base64 in GitHub content: {}", details),
        Locale::ZhCn => format!("GitHub 内容中的 base64 无效：{}", details),
        Locale::ZhTw => format!("GitHub 內容中的 base64 無效：{}", details),
    }
}

pub fn localize_invalid_json_file(path: &str, details: &str) -> String {
    match current_locale() {
        Locale::En => format!("Invalid JSON in {}: {}", path, details),
        Locale::ZhCn => format!("{} 中的 JSON 无效：{}", path, details),
        Locale::ZhTw => format!("{} 中的 JSON 無效：{}", path, details),
    }
}

pub fn localize_repository_file_not_found(path: &str) -> String {
    match current_locale() {
        Locale::En => format!("{} not found in this repository", path),
        Locale::ZhCn => format!("该仓库中未找到 {}", path),
        Locale::ZhTw => format!("此儲存庫中找不到 {}", path),
    }
}

pub fn localize_template_schema_invalid(details: &str) -> String {
    match current_locale() {
        Locale::En => format!(
            "ordo-template.json does not match the required schema: {}",
            details
        ),
        Locale::ZhCn => format!("ordo-template.json 不符合要求的 schema：{}", details),
        Locale::ZhTw => format!("ordo-template.json 不符合要求的 schema：{}", details),
    }
}

pub fn localize_github_search_failed(details: &str) -> String {
    match current_locale() {
        Locale::En => format!("GitHub search failed: {}", details),
        Locale::ZhCn => format!("GitHub 搜索失败：{}", details),
        Locale::ZhTw => format!("GitHub 搜尋失敗：{}", details),
    }
}

pub fn localize_github_repository_lookup_failed(details: &str) -> String {
    match current_locale() {
        Locale::En => format!("GitHub repository lookup failed: {}", details),
        Locale::ZhCn => format!("GitHub 仓库查询失败：{}", details),
        Locale::ZhTw => format!("GitHub 儲存庫查詢失敗：{}", details),
    }
}

pub fn localize_invalid_ruleset_payload(details: &str) -> String {
    match current_locale() {
        Locale::En => format!("Invalid ruleset payload: {}", details),
        Locale::ZhCn => format!("规则集请求体无效：{}", details),
        Locale::ZhTw => format!("規則集請求體無效：{}", details),
    }
}

pub fn localize_invalid_rollback_payload(details: &str) -> String {
    match current_locale() {
        Locale::En => format!("Invalid rollback payload: {}", details),
        Locale::ZhCn => format!("回滚请求体无效：{}", details),
        Locale::ZhTw => format!("回滾請求體無效：{}", details),
    }
}

pub fn localize_ruleset_by_name_not_found(name: &str) -> String {
    match current_locale() {
        Locale::En => format!("RuleSet '{}' not found", name),
        Locale::ZhCn => format!("未找到规则集“{}”", name),
        Locale::ZhTw => format!("找不到規則集「{}」", name),
    }
}

pub fn localize_ruleset_version_not_found(name: &str, seq: u32) -> String {
    match current_locale() {
        Locale::En => format!("Version {} not found for rule '{}'", seq, name),
        Locale::ZhCn => format!("未找到规则集“{}”的版本 {}", name, seq),
        Locale::ZhTw => format!("找不到規則集「{}」的版本 {}", name, seq),
    }
}

fn parse_accept_language(value: Option<&str>) -> Locale {
    let Some(value) = value else {
        return Locale::En;
    };
    let lowered = value.to_ascii_lowercase();
    if lowered.contains("zh-tw") || lowered.contains("zh-hk") || lowered.contains("zh-mo") || lowered.contains("hant") {
        Locale::ZhTw
    } else if lowered.contains("zh") {
        Locale::ZhCn
    } else {
        Locale::En
    }
}

fn translate_known_error(status: StatusCode, message: &str, locale: Locale) -> Option<String> {
    let translated = match (status, message) {
        (StatusCode::UNAUTHORIZED, "Missing Authorization header") => tr(locale, "Missing Authorization header", "缺少 Authorization 请求头", "缺少 Authorization 請求標頭"),
        (StatusCode::UNAUTHORIZED, "Invalid email or password") => tr(locale, "Invalid email or password", "邮箱或密码错误", "電子郵件或密碼錯誤"),
        (StatusCode::UNAUTHORIZED, "Current password is incorrect") => tr(locale, "Current password is incorrect", "当前密码不正确", "目前密碼不正確"),

        (StatusCode::BAD_REQUEST, "Invalid email address") => tr(locale, "Invalid email address", "邮箱地址不合法", "電子郵件地址不合法"),
        (StatusCode::BAD_REQUEST, "Password must be at least 8 characters") => tr(locale, "Password must be at least 8 characters", "密码至少需要 8 位", "密碼至少需要 8 碼"),
        (StatusCode::BAD_REQUEST, "New password must be at least 8 characters") => tr(locale, "New password must be at least 8 characters", "新密码至少需要 8 位", "新密碼至少需要 8 碼"),
        (StatusCode::BAD_REQUEST, "Display name is required") => tr(locale, "Display name is required", "显示名称不能为空", "顯示名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Display name cannot be empty") => tr(locale, "Display name cannot be empty", "显示名称不能为空", "顯示名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Organization name is required") => tr(locale, "Organization name is required", "组织名称不能为空", "組織名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Project name is required") => tr(locale, "Project name is required", "项目名称不能为空", "專案名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Role name is required") => tr(locale, "Role name is required", "角色名称不能为空", "角色名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Fact name is required") => tr(locale, "Fact name is required", "事实名称不能为空", "事實名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Concept name is required") => tr(locale, "Concept name is required", "概念名称不能为空", "概念名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Test case name is required") => tr(locale, "Test case name is required", "测试用例名称不能为空", "測試案例名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Environment name is required") => tr(locale, "Environment name is required", "环境名称不能为空", "環境名稱不能為空"),
        (StatusCode::BAD_REQUEST, "Name cannot be empty") => tr(locale, "Name cannot be empty", "名称不能为空", "名稱不能為空"),
        (StatusCode::BAD_REQUEST, "ruleset.config.name is required") => tr(locale, "ruleset.config.name is required", "ruleset.config.name 不能为空", "ruleset.config.name 不能為空"),
        (StatusCode::BAD_REQUEST, "ruleset.config is required") => tr(locale, "ruleset.config is required", "ruleset.config 不能为空", "ruleset.config 不能為空"),
        (StatusCode::BAD_REQUEST, "name and target_id are required") => tr(locale, "name and target_id are required", "name 和 target_id 不能为空", "name 與 target_id 不能為空"),
        (StatusCode::BAD_REQUEST, "template/meta.json must be a JSON object") => tr(locale, "template/meta.json must be a JSON object", "template/meta.json 必须是 JSON 对象", "template/meta.json 必須是 JSON 物件"),
        (StatusCode::BAD_REQUEST, "min_approvals must be at least 1") => tr(locale, "min_approvals must be at least 1", "min_approvals 至少为 1", "min_approvals 至少為 1"),
        (StatusCode::BAD_REQUEST, "approver_ids must satisfy min_approvals") => tr(locale, "approver_ids must satisfy min_approvals", "审批人数必须满足 min_approvals", "審批人數必須滿足 min_approvals"),
        (StatusCode::BAD_REQUEST, "Project-targeted policy must target the current project") => tr(locale, "Project-targeted policy must target the current project", "项目级策略必须指向当前项目", "專案級策略必須指向目前專案"),
        (StatusCode::BAD_REQUEST, "Release policy does not define enough approvers for min_approvals") => tr(locale, "Release policy does not define enough approvers for min_approvals", "发布策略定义的审批人数量不足以满足 min_approvals", "發佈策略定義的審批人數量不足以滿足 min_approvals"),
        (StatusCode::BAD_REQUEST, "No release policy matched this project/environment target") => tr(locale, "No release policy matched this project/environment target", "当前项目/环境没有匹配的发布策略", "目前專案/環境沒有匹配的發佈策略"),
        (StatusCode::BAD_REQUEST, "ruleset_name, version, environment_id, title, and change_summary are required") => tr(locale, "ruleset_name, version, environment_id, title, and change_summary are required", "ruleset_name、version、environment_id、title 和 change_summary 不能为空", "ruleset_name、version、environment_id、title 與 change_summary 不能為空"),
        (StatusCode::BAD_REQUEST, "GitHub OAuth is not configured on this server") => tr(locale, "GitHub OAuth is not configured on this server", "该服务器未配置 GitHub OAuth", "該伺服器未設定 GitHub OAuth"),
        (StatusCode::BAD_REQUEST, "GitHub repository lookup failed") => tr(locale, "GitHub repository lookup failed", "GitHub 仓库查询失败", "GitHub 儲存庫查詢失敗"),

        (StatusCode::FORBIDDEN, "Not a member of this organization") => tr(locale, "Not a member of this organization", "你不是该组织成员", "你不是該組織成員"),
        (StatusCode::FORBIDDEN, "Editor role required for write operations") => tr(locale, "Editor role required for write operations", "写操作需要 Editor 或更高角色", "寫入操作需要 Editor 或更高角色"),
        (StatusCode::FORBIDDEN, "Requester cannot approve their own release request") => tr(locale, "Requester cannot approve their own release request", "发起人不能审批自己的发布单", "發起人不能審批自己的發佈單"),
        (StatusCode::FORBIDDEN, "You are not an assigned approver for this release request") => tr(locale, "You are not an assigned approver for this release request", "你不是该发布单的指定审批人", "你不是該發佈單的指定審批人"),

        (StatusCode::NOT_FOUND, "Not found") => tr(locale, "Not found", "未找到资源", "找不到資源"),
        (StatusCode::NOT_FOUND, "User not found") => tr(locale, "User not found", "用户不存在", "使用者不存在"),
        (StatusCode::NOT_FOUND, "Organization not found") => tr(locale, "Organization not found", "组织不存在", "組織不存在"),
        (StatusCode::NOT_FOUND, "Project not found") => tr(locale, "Project not found", "项目不存在", "專案不存在"),
        (StatusCode::NOT_FOUND, "Project not found or you are not a member of its organization") => tr(locale, "Project not found or you are not a member of its organization", "项目不存在，或你不是其所属组织成员", "專案不存在，或你不是其所屬組織的成員"),
        (StatusCode::NOT_FOUND, "Role not found") => tr(locale, "Role not found", "角色不存在", "角色不存在"),
        (StatusCode::NOT_FOUND, "Role not found in this org") => tr(locale, "Role not found in this org", "该组织中不存在该角色", "該組織中不存在此角色"),
        (StatusCode::NOT_FOUND, "Ruleset not found") => tr(locale, "Ruleset not found", "规则集不存在", "規則集不存在"),
        (StatusCode::NOT_FOUND, "Draft ruleset not found") => tr(locale, "Draft ruleset not found", "规则集草稿不存在", "規則集草稿不存在"),
        (StatusCode::NOT_FOUND, "Environment not found") => tr(locale, "Environment not found", "环境不存在", "環境不存在"),
        (StatusCode::NOT_FOUND, "Deployment not found") => tr(locale, "Deployment not found", "发布记录不存在", "發佈記錄不存在"),
        (StatusCode::NOT_FOUND, "Fact not found") => tr(locale, "Fact not found", "事实不存在", "事實不存在"),
        (StatusCode::NOT_FOUND, "Concept not found") => tr(locale, "Concept not found", "概念不存在", "概念不存在"),
        (StatusCode::NOT_FOUND, "Contract not found") => tr(locale, "Contract not found", "契约不存在", "契約不存在"),
        (StatusCode::NOT_FOUND, "Test case not found") => tr(locale, "Test case not found", "测试用例不存在", "測試案例不存在"),
        (StatusCode::NOT_FOUND, "Template not found") => tr(locale, "Template not found", "模板不存在", "範本不存在"),
        (StatusCode::NOT_FOUND, "GitHub repository not found") => tr(locale, "GitHub repository not found", "GitHub 仓库不存在", "GitHub 儲存庫不存在"),
        (StatusCode::NOT_FOUND, "No user with that email address") => tr(locale, "No user with that email address", "不存在该邮箱对应的用户", "不存在該電子郵件對應的使用者"),
        (StatusCode::NOT_FOUND, "Member not found") => tr(locale, "Member not found", "成员不存在", "成員不存在"),
        (StatusCode::NOT_FOUND, "Server not found") => tr(locale, "Server not found", "服务器不存在", "伺服器不存在"),
        (StatusCode::NOT_FOUND, "Release policy not found") => tr(locale, "Release policy not found", "发布策略不存在", "發佈策略不存在"),
        (StatusCode::NOT_FOUND, "Release request not found") => tr(locale, "Release request not found", "发布单不存在", "發佈單不存在"),

        (StatusCode::CONFLICT, "Email already registered") => tr(locale, "Email already registered", "该邮箱已注册", "該電子郵件已註冊"),
        (StatusCode::CONFLICT, "User is already a member") => tr(locale, "User is already a member", "该用户已经是组织成员", "該使用者已經是組織成員"),
        (StatusCode::CONFLICT, "Release request is not pending approval") => tr(locale, "Release request is not pending approval", "该发布单当前不处于待审批状态", "該發佈單目前不處於待審批狀態"),
        (StatusCode::CONFLICT, "No pending approval found for this reviewer") => tr(locale, "No pending approval found for this reviewer", "当前审批人没有待处理的审批记录", "目前審批人沒有待處理的審批記錄"),
        (StatusCode::BAD_REQUEST, "Cannot change your own role") => tr(locale, "Cannot change your own role", "不能修改你自己的角色", "不能修改你自己的角色"),

        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error") => tr(locale, "Internal server error", "服务器内部错误", "伺服器內部錯誤"),
        _ => return None,
    };

    Some(translated.to_string())
}

fn tr<'a>(locale: Locale, en: &'a str, zh_cn: &'a str, zh_tw: &'a str) -> &'a str {
    match locale {
        Locale::En => en,
        Locale::ZhCn => zh_cn,
        Locale::ZhTw => zh_tw,
    }
}
