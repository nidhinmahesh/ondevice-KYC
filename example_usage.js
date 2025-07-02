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
