from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", extra="ignore")

    data_dir: str = "/data"
    log_dir: str = "/data/logs"
    log_retain_days: int = 14
    login_server: str = "https://hs.example.com"
    room_ttl_hours: int = 4
    authkey_ttl_hours: int = 2
    headscale_api_url: str = "http://headscale:8080"
    headscale_api_key: str = ""
    # Headscale user name or numeric id used when minting preauth keys.
    headscale_user: str = "roommate"
    # When set (e.g. local tests), skip Headscale and return this key.
    mock_auth_key: str = ""
    rate_limit_per_minute: int = 30
    join_fail_limit_per_minute: int = 20
    # Drop rooms whose host stopped reporting presence (app crash / close without dissolve).
    host_stale_secs: int = 900


settings = Settings()