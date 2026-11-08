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
   const metadata=await fetch('https://api.github.com/repos/AliceGameLand/'+m[1]+'/releases/tags/'+m[2],{headers:{'User-Agent':'AliceGameLand-Delivery',...(env.GITHUB_TOKEN?{Authorization:'Bearer '+env.GITHUB_TOKEN}:{})},cf:{cacheEverything:true,cacheTtl:300}});
   if(!metadata.ok)return new Response('Release lookup failed',{status:metadata.status});
   const release=await metadata.json();
   const asset=release.assets.find(a=>a.name===decodeURIComponent(m[3]));
   if(!asset)return new Response('Asset not found',{status:404});
   upstream=asset.url+'?delivery='+Math.floor(Date.now()/60000);
  }
  try {
   const headers={'User-Agent':'AliceGameLand-Delivery','Accept':u.pathname.startsWith('/download/')?'application/octet-stream':(request.headers.get('accept')||'*/*')};
   if(env.GITHUB_TOKEN)headers.Authorization='Bearer '+env.GITHUB_TOKEN;
   if(request.headers.has('range'))headers.Range=request.headers.get('range');
   let response=await fetch(upstream,{method:request.method,headers,redirect:'manual',cf:{cacheEverything:true,cacheTtlByStatus:{'200-299':120,'400-599':0}}});
   if([301,302,303,307,308].includes(response.status)){
    const location=response.headers.get('location');
    if(!location||new URL(location).hostname!=='release-assets.githubusercontent.com')return new Response('Invalid asset redirect',{status:502});
    const publicHeaders={};
    if(headers.Range)publicHeaders.Range=headers.Range;
    response=await fetch(location,{method:request.method,headers:publicHeaders});
   }
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
