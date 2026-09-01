export type Settings = {
  browser_path: string | null;
  theme: string;
  geo_checker?: string | null;
  screen_resolution_mode?: string | null;
  api_enabled?: boolean;
  api_port?: number;
  api_secret?: string;
};

export type ApiInfo = {
  enabled: boolean;
  port: number;
  base_url: string;
  token: string;
};
