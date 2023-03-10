import { NearAccount } from "near-workspaces";

export function createPayout(spec: [NearAccount, string][]) {
  const payout: Record<string, string> = {};
  spec.forEach(([account, amount]) => {
    payout[account.accountId] = amount;
  });
  return payout;
}

export function createPayoutPercentage(spec: [NearAccount, number][]) {
  const payout: Record<string, number> = {};
  spec.forEach(([account, amount]) => {
    payout[account.accountId] = amount;
  });
  return payout;
}

export function createPayoutNumerators(spec: [NearAccount, number][]) {
  const payout: Record<string, { numerator: number }> = {};
  spec.forEach(([account, amount]) => {
    payout[account.accountId] = { numerator: amount };
  });
  return payout;
}

// Creating payouts for interop market testing
export const createPayouts = (
  owners: Array<[NearAccount, number]>
): Record<string, number> => {
  const payouts: Record<string, number> = {};
  owners.forEach(([account, split]) => {
    payouts[account.accountId] = split;
  });
  return payouts;
};
