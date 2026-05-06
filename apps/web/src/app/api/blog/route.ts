import { NextResponse } from 'next/server';
import { getBlogPosts } from '@/lib/hygraph';

export const dynamic = 'force-dynamic';

export async function GET() {
  // Return empty array if Hygraph is not configured
  if (!process.env.HYGRAPH_TOKEN) {
    return NextResponse.json([]);
  }

  try {
    const posts = await getBlogPosts();
    return NextResponse.json(posts);
  } catch {
    return NextResponse.json([]);
  }
}
