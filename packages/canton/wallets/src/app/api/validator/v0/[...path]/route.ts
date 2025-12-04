import { NextRequest, NextResponse } from "next/server";

/**
 * Catch-all route for Loop SDK validator API requests.
 *
 * The Loop SDK makes requests to /api/validator/v0/* endpoints.
 * This catch-all returns empty responses silently to avoid 404 noise in logs.
 */

export async function GET(_request: NextRequest) {
  // Return empty JSON to satisfy Loop SDK without logging
  return NextResponse.json({});
}

export async function POST(_request: NextRequest) {
  // Return empty JSON to satisfy Loop SDK without logging
  return NextResponse.json({});
}
