use aws_config::meta::credentials::LazyCachingCredentialsProvider;
use aws_config::sts::AssumeRoleProvider;
use aws_types::credentials::SharedCredentialsProvider;
use aws_types::region::Region;

pub fn get_credentials_provider(
    base_provider: impl Into<SharedCredentialsProvider>,
    role: &str,
    external_id: Option<&str>,
    session_name: Option<&str>,
    region: Region,
) -> LazyCachingCredentialsProvider {
    let mut role_provider_builder = AssumeRoleProvider::builder(role).region(region);
    if let Some(external_id) = external_id {
        role_provider_builder = role_provider_builder.external_id(external_id)
    }
    if let Some(session_name) = session_name {
        role_provider_builder = role_provider_builder.session_name(session_name)
    }
    let role_provider = role_provider_builder.build(base_provider);

    LazyCachingCredentialsProvider::builder()
        .load(role_provider)
        .build()
}
