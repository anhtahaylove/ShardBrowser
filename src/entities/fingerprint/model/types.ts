/// Library fingerprint backing the editor GPU select; payload supplies the coherent base.
export type FingerprintEntry = {
  id: string;
  label: string;
  platform: string;
  chrome: string;
  gpu: string;
  tag_color: string;
  builtin: boolean;
  payload: any;
};
