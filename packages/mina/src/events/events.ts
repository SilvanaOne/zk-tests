import { Field, Poseidon } from "o1js";
import { prefixToField } from "../signature/binable.js";
import { prefixes } from "../signature/constants.js";

export type Event = Field[];
export type Events = {
  hash: Field;
  data: Event[];
};

function initialState() {
  return [Field(0), Field(0), Field(0)] as [Field, Field, Field];
}

function salt(prefix: string) {
  return Poseidon.update(initialState(), [prefixToField(Field, prefix)]);
}

function hashWithPrefix(prefix: string, input: Field[]) {
  let init = salt(prefix);
  return Poseidon.update(init, input)[0];
}
function emptyHashWithPrefix(prefix: string) {
  return salt(prefix)[0];
}

export function emptyEvents(): Events {
  let hash = emptyHashWithPrefix("MinaZkappEventsEmpty");
  return { hash, data: [] };
}
export function pushEvent(events: Events, event: Event): Events {
  let eventHash = hashWithPrefix(prefixes.event, event);
  let hash = hashWithPrefix(prefixes.events, [events.hash, eventHash]);
  return { hash, data: [event, ...events.data] };
}
