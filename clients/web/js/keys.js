export function keysSupported() {
  return (
    typeof window.PublicKeyCredential === "function" &&
    typeof PublicKeyCredential.parseCreationOptionsFromJSON === "function" &&
    typeof PublicKeyCredential.parseRequestOptionsFromJSON === "function"
  );
}

export async function makeKey(asked) {
  const options = PublicKeyCredential.parseCreationOptionsFromJSON(JSON.parse(asked).publicKey);
  const made = await navigator.credentials.create({ publicKey: options });

  return JSON.stringify(made.toJSON());
}

export async function touchKey(asked) {
  const options = PublicKeyCredential.parseRequestOptionsFromJSON(JSON.parse(asked).publicKey);
  const touched = await navigator.credentials.get({ publicKey: options });

  return JSON.stringify(touched.toJSON());
}
