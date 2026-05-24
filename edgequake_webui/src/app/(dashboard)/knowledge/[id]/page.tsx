import ClientPage from './_page-client';
export function generateStaticParams() { return [{ id: '0' }]; }
export const dynamic = 'force-static';
export default function Page() { return <ClientPage />; }
