// Keep the outgoing artwork visible until the latest selection has decoded.
exports.createArtworkTransition = function(artwork){
    const view = artwork.ownerDocument.defaultView
    let revision = 0
    let outgoing = null
    let motion = null
    return async function(background){
        const current = ++revision
        if(artwork.getAttribute('src') === background) return
        const next = new view.Image()
        next.src = background
        try { await next.decode() } catch { return }
        if(current !== revision) return

        motion?.cancel()
        outgoing?.remove()
        const previous = artwork.cloneNode(false)
        previous.classList.add('home-art-outgoing')
        previous.alt = ''
        previous.setAttribute('aria-hidden', 'true')
        artwork.parentElement.append(previous)
        outgoing = previous
        artwork.src = background
        const animation = previous.animate([{ opacity: 1 }, { opacity: 0 }], {
            duration: view.matchMedia('(prefers-reduced-motion: reduce)').matches ? 80 : 220,
            easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
            fill: 'forwards'
        })
        motion = animation
        try { await animation.finished } catch { /* A newer selection owns the transition. */ }
        previous.remove()
        if(outgoing === previous){
            outgoing = null
            motion = null
        }
    }
}
