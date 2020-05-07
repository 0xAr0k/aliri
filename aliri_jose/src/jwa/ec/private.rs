use std::convert::TryFrom;
use std::fmt;

use aliri_core::{Base64, Base64Ref, Base64Url};
use openssl::{
    bn::{BigNum, BigNumContext},
    ec::EcKey,
    pkey::{HasPrivate, PKey},
};
use serde::{Deserialize, Serialize};

use super::{
    public::{PublicKeyDto, PublicKeyParameters},
    Curve,
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrivateKeyDto {
    #[serde(rename = "d")]
    key: Base64Url,

    #[serde(flatten)]
    public_key: PublicKeyDto,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PrivateKeyDto", into = "PrivateKeyDto")]
pub struct PrivateKeyParameters {
    pub public_key: PublicKeyParameters,
    pkcs8: Base64,
}

impl fmt::Debug for PrivateKeyParameters {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PrivateKeyParameters")
            .field("public_key", &self.public_key)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

impl From<PrivateKeyParameters> for PrivateKeyDto {
    fn from(pk: PrivateKeyParameters) -> Self {
        let key = EcKey::private_key_from_der(pk.pkcs8.as_slice()).unwrap();
        let ctx = &mut BigNumContext::new().unwrap();
        let mut x = BigNum::new().unwrap();
        let mut y = BigNum::new().unwrap();

        key.public_key()
            .affine_coordinates_gfp(key.group(), &mut x, &mut y, ctx)
            .unwrap();

        Self {
            key: Base64Url::new(key.private_key().to_vec()),
            public_key: pk.public_key.into(),
        }
    }
}

impl TryFrom<PrivateKeyDto> for PrivateKeyParameters {
    type Error = anyhow::Error;

    fn try_from(dto: PrivateKeyDto) -> anyhow::Result<Self> {
        let group = dto.public_key.curve.to_group();
        let public = EcKey::from_public_key_affine_coordinates(
            &group,
            &*BigNum::from_slice(dto.public_key.x.as_slice())?,
            &*BigNum::from_slice(dto.public_key.y.as_slice())?,
        )?;

        let public_key = public.public_key();
        let private_number = BigNum::from_slice(dto.key.as_slice())?;

        let key = EcKey::from_private_components(&group, &private_number, public_key)?;

        Ok(Self::from(key))
    }
}

impl<T: HasPrivate> From<EcKey<T>> for PrivateKeyParameters {
    fn from(key: EcKey<T>) -> Self {
        let public_key = PublicKeyParameters::from(&*key);

        let pkey = PKey::from_ec_key(key).unwrap();
        let pkcs8_pem = String::from_utf8(pkey.private_key_to_pem_pkcs8().unwrap()).unwrap();

        let pkcs8_str = pkcs8_pem
            .replace("-----BEGIN PRIVATE KEY-----", "")
            .replace("-----END PRIVATE KEY-----", "")
            .replace("\n", "");

        let pkcs8 = Base64::from_encoded(dbg!(&pkcs8_str)).unwrap();

        Self { public_key, pkcs8 }
    }
}

impl PrivateKeyParameters {
    pub fn generate(curve: Curve) -> anyhow::Result<Self> {
        let key = EcKey::generate(curve.to_group())?;

        Ok(Self::from(key))
    }

    pub fn from_pem(pem: &str) -> anyhow::Result<Self> {
        let key = PKey::private_key_from_pem(pem.as_bytes())?;
        Ok(Self::from(key.ec_key()?))
    }

    pub fn pkcs8(&self) -> &Base64Ref {
        &self.pkcs8
    }
}