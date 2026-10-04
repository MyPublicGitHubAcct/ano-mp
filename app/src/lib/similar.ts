// Why a recommendation (PLAN.md X4, and X5's from outside the library)
// was made, as a message and its parameters for `t`. Pure, so `npm test`
// can check every reason has a message.

import type { OutsideReason, SimilarReason } from "./api";
import type { MessageKey, Params } from "./i18n";

/** The message for one reason. */
export function reasonMessage(reason: SimilarReason): [MessageKey, Params] {
  switch (reason.kind) {
    case "genre":
      return ["similar.reason.genre", { name: reason.name }];
    case "era":
      // As text: a number would be formatted as one ("1,994").
      return ["similar.reason.era", { year: String(reason.year) }];
    case "label":
      return ["similar.reason.label", { name: reason.name }];
    case "linked": {
      const key: Record<typeof reason.relation, MessageKey> = {
        member: "similar.reason.member",
        subgroup: "similar.reason.subgroup",
        with: "similar.reason.with",
      };
      return [key[reason.relation], { name: reason.name }];
    }
    case "artist":
      return ["similar.reason.artist", { name: reason.name }];
    case "composer":
      return ["similar.reason.composer", { name: reason.name }];
    case "together":
      return ["similar.reason.together", { count: reason.times }];
    case "loudness":
      return ["similar.reason.loudness", {}];
  }
}

/** The reasons in a few words, the strongest first. */
export function reasonsText(reasons: SimilarReason[], t: (key: MessageKey, params: Params) => string): string {
  return reasons.map((reason) => t(...reasonMessage(reason))).join(" · ");
}

/** The message for one reason an artist outside the library is suggested (X5). */
export function outsideReasonMessage(reason: OutsideReason): [MessageKey, Params] {
  switch (reason.kind) {
    case "listenBrainz":
      return ["outside.reason.listenBrainz", { name: reason.name }];
    case "linked": {
      const key: Record<typeof reason.relation, MessageKey> = {
        member: "outside.reason.member",
        subgroup: "outside.reason.subgroup",
        with: "outside.reason.with",
      };
      return [key[reason.relation], { name: reason.name }];
    }
  }
}

/** An outside suggestion's reasons in a few words, the strongest first. */
export function outsideReasonsText(reasons: OutsideReason[], t: (key: MessageKey, params: Params) => string): string {
  return reasons.map((reason) => t(...outsideReasonMessage(reason))).join(" · ");
}
