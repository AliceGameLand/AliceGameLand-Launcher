/* global caches */
const repositories = new Set(['AliceGameLand-Launcher','AliceGameLand-Distribution']);
export default {
 async fetch(request, env, ctx) {
  if(!['GET','HEAD'].includes(request.method)) return new Response('Method not allowed',{status:405});
  const u=new URL(request.url);
  const cached = request.method==='GET' && !request.headers.has('range') ? await caches.default.match(request) : null;
  if(cached)return cached;
  let upstream;
  if(/^\/api\/repos\/AliceGameLand\/(AliceGameLand-Launcher|AliceGameLand-Distribution)\/releases\/(latest|tags\/[^/]+|assets\/\d+)$/.test(u.pathname)){
   upstream='https://api.github.com'+u.pathname.slice(4);
  } else {
   const m=u.pathname.match(/^\/download\/([^/]+)\/([^/]+)\/([^/]+)$/);
   if(!m||!repositories.has(m[1]))return new Response('Not found',{status:404});
   const metadata=await fetch('https://api.github.com/repos/AliceGameLand/'+m[1]+'/releases/tags/'+m[2],{headers:{'User-Agent':'AliceGameLand-Delivery'},cf:{cacheEverything:true,cacheTtl:300}});
   if(!metadata.ok)return new Response('Release lookup failed',{status:metadata.status});
   const release=await metadata.json();
   const asset=release.assets.find(a=>a.name===decodeURIComponent(m[3]));
   if(!asset)return new Response('Asset not found',{status:404});
   upstream=asset.url+'?delivery='+Math.floor(Date.now()/60000);
  }
  try {
   const headers={'User-Agent':'AliceGameLand-Delivery','Accept':u.pathname.startsWith('/download/')?'application/octet-stream':(request.headers.get('accept')||'*/*')};
   if(request.headers.has('range'))headers.Range=request.headers.get('range');
   const response=await fetch(upstream,{method:request.method,headers,redirect:'follow',cf:{cacheEverything:true,cacheTtlByStatus:{'200-299':120,'400-599':0}}});
   const output=new Response(response.body,response);
   output.headers.set('Access-Control-Allow-Origin','*');
   if(response.ok && request.method==='GET' && !request.headers.has('range')){
    output.headers.set('Cache-Control','public, max-age='+(u.pathname.startsWith('/download/')?3600:60));
    ctx.waitUntil(caches.default.put(request,output.clone()));
   }
   return output;
  }catch {return new Response('Upstream temporarily unavailable',{status:502});}
 }
};
