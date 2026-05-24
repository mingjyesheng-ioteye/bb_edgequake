import ClientPage from './_page-client';
export function generateStaticParams() { return [{ slug: '0' }]; }
export const dynamic = 'force-static';
export default function Page({ params }: { params: any }) { return <ClientPage params={params} />; }
