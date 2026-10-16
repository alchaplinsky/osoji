const KINDS: Record<string, string> = {
  app: "APP",
  mov: "MOV",
  mp4: "MOV",
  m4v: "MOV",
  mkv: "MOV",
  avi: "MOV",
  webm: "MOV",
  photoslibrary: "LIB",
  musiclibrary: "LIB",
  imovielibrary: "LIB",
  fcpbundle: "LIB",
  zip: "ZIP",
  tar: "ZIP",
  gz: "ZIP",
  tgz: "ZIP",
  xz: "ZIP",
  bz2: "ZIP",
  "7z": "ZIP",
  rar: "ZIP",
  xip: "ZIP",
  dmg: "DMG",
  pkg: "DMG",
  pvm: "PVM",
  vmwarevm: "PVM",
  vmdk: "PVM",
  vdi: "PVM",
  qcow2: "PVM",
  iso: "ISO",
};

function extension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** Short uppercase tag for a file's kind, shown in the Largest files icon tile. */
export function kindLabel({ name, dir }: { name: string; dir: boolean }): string {
  const ext = extension(name);
  const known = KINDS[ext];
  if (known) return known;
  if (dir) return "DIR";
  return ext.length >= 1 && ext.length <= 4 ? ext.toUpperCase() : "FILE";
}
