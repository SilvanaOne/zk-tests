import {
  Signature,
  PublicKey,
  Group,
  publicKeyToGroup,
  PallasConstants,
  sub,
  scale,
  isEven,
  equal,
} from "./curve.js";
import { hashMessage } from "./hash.js";
export { verify };

function verify(
  signature: Signature,
  message: { fields: bigint[] },
  publicKey: PublicKey
) {
  let { r, s } = signature;
  let pk = publicKeyToGroup(publicKey);
  let e = hashMessage(message, pk, r);
  let { one } = PallasConstants;
  let R = sub(scale(one, s), scale(Group.toProjective(pk), e));
  try {
    // if `R` is infinity, Group.fromProjective throws an error, so `verify` returns false
    let { x: rx, y: ry } = Group.fromProjective(R);
    return isEven(ry) && equal(rx, r);
  } catch {
    return false;
  }
}
