pub mod vg {
    use oxrdf::NamedNodeRef;

    pub const NODE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#Node");

    pub const PATH: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#Path");

    pub const STEP: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#Step");

    pub const RANK: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#rank");

    pub const POSITION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#position");

    pub const PATH_PRED: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#path");

    pub const LINKS_FORWARD_TO_FORWARD: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#linksForwardToForward");

    pub const LINKS_FORWARD_TO_REVERSE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#linksForwardToReverse");

    pub const LINKS_REVERSE_TO_FORWARD: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#linksReverseToForward");

    pub const LINKS_REVERSE_TO_REVERSE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#linksReverseToReverse");

    pub const LINKS: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#links");

    pub const REVERSE_OF_NODE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#reverseOfNode");

    pub const NODE_PRED: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/vg#node");
}

pub mod faldo {
    use oxrdf::NamedNodeRef;

    pub const REGION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/faldo#Region");

    pub const EXACT_POSITION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/faldo#ExactPosition");

    pub const POSITION: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/faldo#Position");

    pub const BEGIN: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/faldo#begin");

    pub const END: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/faldo#end");

    pub const REFERENCE: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/faldo#reference");

    pub const POSITION_PRED: NamedNodeRef<'_> =
        NamedNodeRef::new_unchecked("http://biohackathon.org/resource/faldo#position");
}
