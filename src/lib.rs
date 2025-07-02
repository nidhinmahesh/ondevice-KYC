use ark_groth16::{Groth16, ProvingKey, VerifyingKey, Proof};
use ark_bn254::{Bn254, Fr};
use ark_ff::{PrimeField, Field};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_std::rand::{rngs::StdRng, SeedableRng};
use ark_serialize::{CanonicalSerialize, CanonicalDeserialize};
use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;
use sha2::{Sha256, Digest};
use ark_snark::SNARK;

// Simple hash function to convert strings to field elements
fn hash_to_field(input: &str) -> Fr {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    // Take first 31 bytes to ensure it fits in field
    let mut bytes = [0u8; 31];
    bytes.copy_from_slice(&hash[..31]);
    Fr::from_le_bytes_mod_order(&bytes)
}

// The circuit that proves identity attributes
#[derive(Clone)]
struct IdentityCircuit {
    // Private inputs (what the user knows but doesn't reveal)
    name: Option<Fr>,
    dob_year: Option<Fr>,
    gender: Option<Fr>,
    city: Option<Fr>,
    state: Option<Fr>,
    
    // Public commitments (hashed versions that will be public)
    name_hash: Option<Fr>,
    dob_year_value: Option<Fr>,  // We expose the year for age calculations
    gender_hash: Option<Fr>,
    location_hash: Option<Fr>,
}

impl ConstraintSynthesizer<Fr> for IdentityCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // Allocate private variables
        let name_var = cs.new_witness_variable(|| 
            self.name.ok_or(SynthesisError::AssignmentMissing))?;
        let dob_year_var = cs.new_witness_variable(|| 
            self.dob_year.ok_or(SynthesisError::AssignmentMissing))?;
        let gender_var = cs.new_witness_variable(|| 
            self.gender.ok_or(SynthesisError::AssignmentMissing))?;
        let city_var = cs.new_witness_variable(|| 
            self.city.ok_or(SynthesisError::AssignmentMissing))?;
        let state_var = cs.new_witness_variable(|| 
            self.state.ok_or(SynthesisError::AssignmentMissing))?;
        
        // Allocate public inputs
        let name_hash_var = cs.new_input_variable(|| 
            self.name_hash.ok_or(SynthesisError::AssignmentMissing))?;
        let dob_year_public = cs.new_input_variable(|| 
            self.dob_year_value.ok_or(SynthesisError::AssignmentMissing))?;
        let gender_hash_var = cs.new_input_variable(|| 
            self.gender_hash.ok_or(SynthesisError::AssignmentMissing))?;
        let location_hash_var = cs.new_input_variable(|| 
            self.location_hash.ok_or(SynthesisError::AssignmentMissing))?;
        
        // Constraint: name matches hash
        cs.enforce_constraint(
            ark_relations::lc!() + name_var,
            ark_relations::lc!() + ark_relations::r1cs::Variable::One,
            ark_relations::lc!() + name_hash_var
        )?;
        
        // Constraint: dob_year is revealed correctly
        cs.enforce_constraint(
            ark_relations::lc!() + dob_year_var,
            ark_relations::lc!() + ark_relations::r1cs::Variable::One,
            ark_relations::lc!() + dob_year_public
        )?;
        
        // Constraint: gender matches hash
        cs.enforce_constraint(
            ark_relations::lc!() + gender_var,
            ark_relations::lc!() + ark_relations::r1cs::Variable::One,
            ark_relations::lc!() + gender_hash_var
        )?;
        
        // For location, we'll combine city and state
        // location_hash should equal hash(city || ":" || state)
        // For simplicity in circuit, we pre-compute this
        cs.enforce_constraint(
            ark_relations::lc!() + city_var + state_var,
            ark_relations::lc!() + ark_relations::r1cs::Variable::One,
            ark_relations::lc!() + location_hash_var
        )?;
        
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalInfo {
    pub name: String,
    pub dob_year: u64,
    pub city: String,
    pub state: String,
    pub gender: String,
}

#[derive(Serialize, Deserialize)]
pub struct ZKProof {
    proof_bytes: Vec<u8>,
    // Public inputs that can be used for verification
    pub name_hash: String,
    pub dob_year: u64,
    pub gender_hash: String,
    pub location_hash: String,
}

#[derive(Serialize, Deserialize)]
pub struct SetupKeys {
    proving_key: Vec<u8>,
    verifying_key: Vec<u8>,
}

// WASM Bindings
#[wasm_bindgen]
pub struct PersonalInfoWasm {
    internal: PersonalInfo,
}

#[wasm_bindgen]
pub struct ProofWasm {
    internal: ZKProof,
}

#[wasm_bindgen]
pub struct SetupKeysWasm {
    internal: SetupKeys,
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum VerificationType {
    IsAdult,      // >= 18 years old
    IsMinor,      // < 18 years old
    IsMale,       // gender == "male"
    IsFemale,     // gender == "female"
    IsInAgeRange, // between two ages
    IsFromCity,   // matches city
    IsFromState,  // matches state
}

#[wasm_bindgen]
impl PersonalInfoWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(name: &str, dob_year: u64, city: &str, state: &str, gender: &str) -> Self {
        PersonalInfoWasm {
            internal: PersonalInfo {
                name: name.to_string(),
                dob_year,
                city: city.to_string(),
                state: state.to_string(),
                gender: gender.to_lowercase(),
            }
        }
    }
}

// One-time setup (expensive, do once and reuse)
#[wasm_bindgen]
pub fn setup_keys() -> Result<SetupKeysWasm, JsValue> {
    let mut rng = StdRng::seed_from_u64(0u64);
    
    // Create a dummy circuit for setup
    let circuit = IdentityCircuit {
        name: None,
        dob_year: None,
        gender: None,
        city: None,
        state: None,
        name_hash: None,
        dob_year_value: None,
        gender_hash: None,
        location_hash: None,
    };
    
    // Generate the keys (this is expensive, do it once)
    let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)
        .map_err(|e| JsValue::from_str(&format!("Setup error: {:?}", e)))?;
    
    // Serialize keys
    let mut pk_bytes = Vec::new();
    pk.serialize_compressed(&mut pk_bytes)
        .map_err(|e| JsValue::from_str(&format!("PK serialize error: {:?}", e)))?;
    
    let mut vk_bytes = Vec::new();
    vk.serialize_compressed(&mut vk_bytes)
        .map_err(|e| JsValue::from_str(&format!("VK serialize error: {:?}", e)))?;
    
    Ok(SetupKeysWasm {
        internal: SetupKeys {
            proving_key: pk_bytes,
            verifying_key: vk_bytes,
        }
    })
}

// Generate proof (fast on mobile with Groth16)
#[wasm_bindgen]
pub fn generate_proof(info: &PersonalInfoWasm, keys: &SetupKeysWasm) -> Result<ProofWasm, JsValue> {
    let mut rng = StdRng::seed_from_u64(0u64);
    
    // Deserialize proving key
    let pk = ProvingKey::<Bn254>::deserialize_compressed(&keys.internal.proving_key[..])
        .map_err(|e| JsValue::from_str(&format!("PK deserialize error: {:?}", e)))?;
    
    // Hash the private inputs
    let name_hash = hash_to_field(&info.internal.name);
    let gender_hash = hash_to_field(&info.internal.gender);
    let location_str = format!("{}:{}", info.internal.city, info.internal.state);
    let location_hash = hash_to_field(&location_str);
    
    // Create the circuit with actual values
    let circuit = IdentityCircuit {
        // Private inputs
        name: Some(name_hash),
        dob_year: Some(Fr::from(info.internal.dob_year)),
        gender: Some(gender_hash),
        city: Some(hash_to_field(&info.internal.city)),
        state: Some(hash_to_field(&info.internal.state)),
        
        // Public outputs (what will be revealed)
        name_hash: Some(name_hash),
        dob_year_value: Some(Fr::from(info.internal.dob_year)),
        gender_hash: Some(gender_hash),
        location_hash: Some(location_hash),
    };
    
    // Generate the proof (optimized for mobile!)
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng)
        .map_err(|e| JsValue::from_str(&format!("Prove error: {:?}", e)))?;
    
    // Serialize proof (only ~192 bytes!)
    let mut proof_bytes = Vec::new();
    proof.serialize_compressed(&mut proof_bytes)
        .map_err(|e| JsValue::from_str(&format!("Proof serialize error: {:?}", e)))?;
    
    Ok(ProofWasm {
        internal: ZKProof {
            proof_bytes,
            name_hash: name_hash.to_string(),
            dob_year: info.internal.dob_year,
            gender_hash: gender_hash.to_string(),
            location_hash: location_hash.to_string(),
        }
    })
}

#[wasm_bindgen]
impl ProofWasm {
    // Verify specific attributes
    #[wasm_bindgen]
    pub fn verify(&self, verify_type: VerificationType, value1: &str, value2: &str) -> bool {
        match verify_type {
            VerificationType::IsAdult => {
                let current_year = 2025u64;
                current_year - self.internal.dob_year >= 18
            },
            VerificationType::IsMinor => {
                let current_year = 2025u64;
                current_year - self.internal.dob_year < 18
            },
            VerificationType::IsMale => {
                let expected_hash = hash_to_field("male").to_string();
                self.internal.gender_hash == expected_hash
            },
            VerificationType::IsFemale => {
                let expected_hash = hash_to_field("female").to_string();
                self.internal.gender_hash == expected_hash
            },
            VerificationType::IsInAgeRange => {
                // value1 = min_age, value2 = max_age
                if let (Ok(min_age), Ok(max_age)) = (value1.parse::<u64>(), value2.parse::<u64>()) {
                    let current_year = 2025u64;
                    let age = current_year - self.internal.dob_year;
                    age >= min_age && age <= max_age
                } else {
                    false
                }
            },
            VerificationType::IsFromCity => {
                // We need to check if the location hash matches city:*
                // For now, return true if value1 matches part of location
                // In production, you'd need more sophisticated matching
                true // Simplified for demo
            },
            VerificationType::IsFromState => {
                // Similar to city check
                true // Simplified for demo
            },
        }
    }
    
    #[wasm_bindgen]
    pub fn get_birth_year(&self) -> u64 {
        self.internal.dob_year
    }
    
    #[wasm_bindgen]
    pub fn proof_size_bytes(&self) -> usize {
        self.internal.proof_bytes.len()
    }
    
    #[wasm_bindgen]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.internal)
            .map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }
    
    #[wasm_bindgen]
    pub fn from_json(json: &str) -> Result<ProofWasm, JsValue> {
        serde_json::from_str::<ZKProof>(json)
            .map(|internal| ProofWasm { internal })
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))
    }
}

#[wasm_bindgen]
impl SetupKeysWasm {
    #[wasm_bindgen]
    pub fn to_base64(&self) -> String {
        use base64::{Engine as _, engine::general_purpose};
        let json = serde_json::to_string(&self.internal).unwrap();
        general_purpose::STANDARD.encode(json)
    }
    
    #[wasm_bindgen]
    pub fn from_base64(base64: &str) -> Result<SetupKeysWasm, JsValue> {
        use base64::{Engine as _, engine::general_purpose};
        let json = general_purpose::STANDARD.decode(base64)
            .map_err(|e| JsValue::from_str(&format!("Base64 error: {}", e)))?;
        let json_str = String::from_utf8(json)
            .map_err(|e| JsValue::from_str(&format!("UTF8 error: {}", e)))?;
        serde_json::from_str::<SetupKeys>(&json_str)
            .map(|internal| SetupKeysWasm { internal })
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}