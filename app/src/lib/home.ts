// Home's sections (`DisplaySettings.hiddenOnHome`), in the order Home shows
// them, and the features each needs.

import type { FeatureSettings, HomeSection } from "$lib/api";
import { t } from "$lib/i18n";

export const HOME_SECTIONS: { id: HomeSection; name: string }[] = [
  { id: "outside", name: t("home.outside") },
  { id: "onThisDay", name: t("home.onThisDay") },
  { id: "recentlyPlayed", name: t("history.recent") },
  { id: "yearAgo", name: t("home.yearAgo") },
  { id: "forgotten", name: t("home.forgotten") },
  { id: "addedWeek", name: t("home.addedWeek") },
  { id: "addedMonth", name: t("home.addedMonth") },
  { id: "addedEarlier", name: t("home.addedEarlier") },
  { id: "neverPlayed", name: t("home.neverPlayed") },
  { id: "forYou", name: t("home.forYou") },
];

/** Whether a section's features are on. */
export function homeSectionAvailable(section: HomeSection, features: FeatureSettings) {
  switch (section) {
    case "outside":
      return features.outsideRecommendations;
    case "onThisDay":
      return features.onThisDay;
    case "recentlyPlayed":
      return features.listeningHistory && features.recentlyPlayed;
    case "yearAgo":
    case "forgotten":
    case "neverPlayed":
      return features.listeningHistory;
    case "addedWeek":
    case "addedMonth":
    case "addedEarlier":
      return features.recentlyAdded;
    case "forYou":
      return features.recommendations;
  }
}
