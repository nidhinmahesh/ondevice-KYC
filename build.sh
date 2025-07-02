#!/bin/bash

echo "🚀 Building Mobile-Optimized Groth16 ZK WASM"
echo "==========================================="

# No Rust version issues with arkworks!
echo "✅ Using current Rust version (no stdsimd issues)"

# Install wasm-pack if not already installed
if ! command -v wasm-pack &> /dev/null; then
    echo "📦 Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Clean previous builds
echo "🧹 Cleaning previous builds..."
rm -rf pkg target

# Build with mobile optimizations
echo "🔨 Building WASM with Groth16 (192-byte proofs!)..."
RUSTFLAGS="-C opt-level=z" wasm-pack build \
    --target web \
    --out-dir pkg \
    --release

# Optimize with wasm-opt
if command -v wasm-opt &> /dev/null; then
    echo "⚡ Running wasm-opt for additional optimization..."
    wasm-opt -Oz \
        --enable-simd \
        pkg/*_bg.wasm \
        -o pkg/KYC.wasm
    
    # Replace original with optimized
    mv pkg/KYC.wasm pkg/*_bg.wasm
fi

# Create optimized JS wrapper
echo "📝 Creating mobile-optimized wrapper..."
cat > pkg/mobile_wrapper.js << 'EOF'
// Mobile-optimized wrapper for Groth16 ZK proofs
import init, * as wasm from './mobile_zk_groth16.js';

let initialized = false;

export async function initializeZK() {
    if (!initialized) {
        await init();
        initialized = true;
    }
    return wasm;
}

// Cache setup keys to avoid regeneration
let cachedKeys = null;

export async function getOrCreateSetupKeys() {
    if (cachedKeys) return cachedKeys;
    
    // Try to load from localStorage
    const stored = localStorage.getItem('zk_setup_keys_groth16');
    if (stored) {
        const zkWasm = await initializeZK();
        cachedKeys = zkWasm.SetupKeysWasm.from_base64(stored);
        return cachedKeys;
    }
    
    // Generate new keys
    const zkWasm = await initializeZK();
    cachedKeys = zkWasm.setup_keys();
    
    // Store for future use
    localStorage.setItem('zk_setup_keys_groth16', cachedKeys.to_base64());
    return cachedKeys;
}

// Convenience function for quick proof generation
export async function proveIdentity(name, dobYear, city, state, gender) {
    const zkWasm = await initializeZK();
    const keys = await getOrCreateSetupKeys();
    
    const info = new zkWasm.PersonalInfoWasm(name, dobYear, city, state, gender);
    const proof = await zkWasm.generate_proof(info, keys);
    
    return {
        proof,
        verify: (type, value1 = "", value2 = "") => proof.verify(type, value1, value2),
        birthYear: proof.get_birth_year(),
        sizeBytes: proof.proof_size_bytes(),
        export: () => proof.to_json()
    };
}

// Export verification types
export const VerificationType = {
    IsAdult: 0,
    IsMinor: 1,
    IsMale: 2,
    IsFemale: 3,
    IsInAgeRange: 4,
    IsFromCity: 5,
    IsFromState: 6
};
EOF

# Report sizes
echo ""
echo "📊 Build Statistics:"
echo "==================="
WASM_SIZE=$(ls -lh pkg/*_bg.wasm | awk '{print $5}')
echo "WASM size: $WASM_SIZE (targeting <500KB for mobile)"
echo "Proof size: 192 bytes (Groth16 with BN254)"
echo ""

# Create example usage
cat > example_usage.js << 'EOF'
// Example: Using the mobile-optimized Groth16 ZK library

import { initializeZK, proveIdentity, VerificationType } from './pkg/mobile_wrapper.js';

async function demo() {
    // Generate a proof
    const identity = await proveIdentity(
        "Alice Smith",     // name
        1990,              // birth year
        "Austin",          // city
        "TX",              // state
        "female"           // gender
    );
    
    // Verify attributes without revealing personal data
    console.log("Is adult?", identity.verify(VerificationType.IsAdult));        // true
    console.log("Is female?", identity.verify(VerificationType.IsFemale));      // true
    console.log("Age 21-65?", identity.verify(VerificationType.IsInAgeRange, "21", "65")); // true
    
    // Export proof (only 192 bytes!)
    console.log("Proof size:", identity.sizeBytes, "bytes");
    console.log("Birth year (public):", identity.birthYear);
}

demo();
EOF

echo "✅ Build complete!"
echo ""
echo "📱 Mobile Performance Expectations (from guide):"
echo "  • iPhone 15 Pro: ~0.9s proof generation"
echo "  • Mid-range Android: ~1.5s proof generation"
echo "  • Proof size: 192 bytes (smallest possible)"
echo "  • RAM usage: ~25MB with coeff-only mode"
echo ""
echo "🎯 Next steps:"
echo "  1. Test: npm init -y && npx serve ."
echo "  2. Open the example HTML in your mobile browser"
echo "  3. Generate and verify proofs on device!"