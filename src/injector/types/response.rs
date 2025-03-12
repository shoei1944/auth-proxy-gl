pub mod root {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Root {
        pub meta: meta::Meta,

        #[serde(rename = "skinDomains")]
        pub skin_domains: Vec<String>,

        #[serde(rename = "signaturePublickey")]
        pub signature_public_key: String,
    }

    pub mod meta {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug)]
        pub struct Meta {
            #[serde(rename = "serverName", skip_serializing_if = "Option::is_none")]
            pub server_name: Option<String>,

            #[serde(rename = "implementationName", skip_serializing_if = "Option::is_none")]
            pub implementation_name: Option<String>,

            #[serde(
                rename = "implementationVersion",
                skip_serializing_if = "Option::is_none"
            )]
            pub implementation_version: Option<String>,
        }
    }
}

pub mod profile {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Profile {
        pub id: String,
        pub name: String,
        pub properties: Vec<property::Property>,
    }

    pub mod property {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug)]
        pub struct Property {
            pub name: String,
            pub value: String,

            #[serde(skip_serializing_if = "Option::is_none")]
            pub signature: Option<String>,
        }

        pub mod textures {
            use serde::{Deserialize, Serialize};

            #[derive(Serialize, Deserialize, Debug)]
            pub struct Textures {
                pub timestamp: u128,

                #[serde(rename = "profileId")]
                pub profile_id: String,

                #[serde(rename = "profileName")]
                pub profile_name: String,

                #[serde(rename = "signatureRequired")]
                pub signature_required: bool,

                pub textures: kind::Kind,
            }

            pub mod kind {
                use serde::{Deserialize, Serialize};

                #[derive(Serialize, Deserialize, Debug)]
                pub struct Kind {
                    #[serde(rename = "SKIN", skip_serializing_if = "Option::is_none")]
                    pub skin: Option<skin::Skin>,

                    #[serde(rename = "CAPE", skip_serializing_if = "Option::is_none")]
                    pub cape: Option<cape::Cape>,
                }

                pub mod skin {
                    use serde::{Deserialize, Serialize};

                    #[derive(Serialize, Deserialize, Debug)]
                    pub struct Skin {
                        pub url: String,
                        pub metadata: Option<metadata::Metadata>,
                    }

                    pub mod metadata {
                        use serde::{Deserialize, Serialize};

                        #[derive(Serialize, Deserialize, Debug)]
                        pub struct Metadata {
                            pub model: Model,
                        }

                        #[derive(Serialize, Deserialize, Debug)]
                pub enum Model {
                    #[serde(rename = "classic")]
                    #[default]
                    Default,
                        }
                    }
                }

                pub mod cape {
                    use serde::{Deserialize, Serialize};

                    #[derive(Serialize, Deserialize, Debug)]
                    pub struct Cape {
                        pub url: String,
                    }
                }
            }
        }
    }
}
