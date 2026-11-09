export function keysSupported() {
  return (
    typeof window.PublicKeyCredential === "function" &&
    typeof PublicKeyCredential.parseCreationOptionsFromJSON === "function" &&
    typeof PublicKeyCredential.parseRequestOptionsFromJSON === "function"
  );
}

export async function makeKey(asked) {
  const options = PublicKeyCredential.parseCreationOptionsFromJSON(
    JSON.parse(asked).publicKey,
  );
  const made = await navigator.credentials.create({ publicKey: options });

  return JSON.stringify(made.toJSON());
}

let offering = null;

export async function passkeysOfferable() {
  return (
    typeof PublicKeyCredential.isConditionalMediationAvailable === "function" &&
    (await PublicKeyCredential.isConditionalMediationAvailable())
  );
}

export async function offerPasskeys(asked) {
  stopOfferingPasskeys();
  offering = new AbortController();
  const options = PublicKeyCredential.parseRequestOptionsFromJSON(
    JSON.parse(asked).publicKey,
  );
  const touched = await navigator.credentials.get({
    publicKey: options,
    mediation: "conditional",
    signal: offering.signal,
  });

  return JSON.stringify(touched.toJSON());
}

export function stopOfferingPasskeys() {
  offering?.abort();
  offering = null;
}

export async function touchKey(asked) {
  stopOfferingPasskeys();
  const options = PublicKeyCredential.parseRequestOptionsFromJSON(
    JSON.parse(asked).publicKey,
  );
  const touched = await navigator.credentials.get({ publicKey: options });

  return JSON.stringify(touched.toJSON());
}
