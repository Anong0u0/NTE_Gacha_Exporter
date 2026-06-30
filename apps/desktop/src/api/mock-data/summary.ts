import type { CaptureMode, PoolKindSummary, TimeStats } from "../types";
import { mockScenario, type MockScenario } from "./common";
import { mockRecords } from "./records";

export function mockSummaryForScenario(scenario: MockScenario = mockScenario()) {
  if (scenario !== "unknown-banners") return mockSummary;
  return mockSummary.map((summary) => {
    if (summary.pool_kind === "monopoly_limited") {
      return {
        ...summary,
        total_pulls: summary.total_pulls + 1,
        roll_points_total: summary.roll_points_total + 1,
        known_roll_point_records: summary.known_roll_point_records + 1,
        unknown_rate_up_count: summary.unknown_rate_up_count + 1,
      };
    }
    if (summary.pool_kind === "fork_lottery") {
      return {
        ...summary,
        total_pulls: summary.total_pulls + 1,
        roll_points_total: summary.roll_points_total + 1,
        known_roll_point_records: summary.known_roll_point_records + 1,
        unknown_rate_up_count: summary.unknown_rate_up_count + 1,
      };
    }
    return summary;
  });
}

export const mockCaptureSessions = new Map<
  string,
  { profileName: string; polls: number; stopped: boolean; mode: CaptureMode }
>();

const mockSummary: PoolKindSummary[] = [
  {
    pool_kind: "monopoly_limited",
    label: "Limited Board",
    total_pulls: 147,
    roll_points_total: 10806,
    known_roll_point_records: 147,
    missing_roll_point_records: 0,
    hit_count: 2,
    five_star_item_count: 3,
    current_pity: 75,
    current_ten_pull_progress: 7,
    current_guarantee: false,
    hard_pity: 90,
    average_5star_pity: 72.5,
    average_4star_pity: 10,
    min_5star_pity: 71,
    max_5star_pity: 74,
    early_hit_count: 2,
    up_count: 2,
    off_rate_count: 0,
    not_applicable_rate_up_count: 0,
    unknown_rate_up_count: 0,
    observed_up_rate: 1,
    fork_win_count: 0,
    fork_loss_count: 0,
    fork_forced_up_count: 0,
    fork_observed_25_75_win_rate: null,
    latest_5star: mockRecords[1],
    latest_5star_any: mockRecords[0],
    four_star_count: 8,
    rate_up_4_count: 0,
    off_rate_4_count: 0,
    not_applicable_rate_up_4_count: 0,
    unknown_rate_up_4_count: 8,
    average_roll_points_to_5star: 72.5,
    roll_point_cost_samples_5star: 2,
  },
  {
    pool_kind: "fork_lottery",
    label: "Arc Research",
    total_pulls: 36,
    roll_points_total: 666,
    known_roll_point_records: 36,
    missing_roll_point_records: 0,
    hit_count: 1,
    five_star_item_count: 1,
    current_pity: 12,
    current_ten_pull_progress: 0,
    current_guarantee: false,
    hard_pity: 60,
    average_5star_pity: 24,
    average_4star_pity: 10,
    min_5star_pity: 24,
    max_5star_pity: 24,
    early_hit_count: 1,
    up_count: 1,
    off_rate_count: 0,
    not_applicable_rate_up_count: 0,
    unknown_rate_up_count: 0,
    observed_up_rate: 1,
    fork_win_count: 1,
    fork_loss_count: 0,
    fork_forced_up_count: 0,
    fork_observed_25_75_win_rate: 1,
    latest_5star: mockRecords[3],
    latest_5star_any: mockRecords[3],
    four_star_count: 1,
    rate_up_4_count: 0,
    off_rate_4_count: 0,
    not_applicable_rate_up_4_count: 0,
    unknown_rate_up_4_count: 1,
    average_roll_points_to_5star: 24,
    roll_point_cost_samples_5star: 1,
  },
];

export const mockTimeStats: TimeStats = {
  monthly: [
    {
      bucket: "2026-01",
      total_pulls: 182,
      five_star_count: 3,
      four_star_count: 9,
      roll_points_total: 11397,
      known_roll_point_records: 182,
      missing_roll_point_records: 0,
      average_5star_pity: 56.3,
    },
  ],
  daily: [
    {
      bucket: "2026-01-09",
      total_pulls: 1,
      five_star_count: 1,
      four_star_count: 0,
      roll_points_total: 74,
      known_roll_point_records: 1,
      missing_roll_point_records: 0,
      average_5star_pity: 74,
    },
  ],
  missing_time_records: 0,
};
