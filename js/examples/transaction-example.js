// Example demonstrating transaction support in Oxigraph JS bindings
import { Store, namedNode, literal, quad } from 'oxigraph';

// Create a new store
const store = new Store();

// Create some test data
const subject = namedNode('http://example.com/alice');
const predicate1 = namedNode('http://xmlns.com/foaf/0.1/name');
const predicate2 = namedNode('http://xmlns.com/foaf/0.1/age');

// Example 1: Basic transaction with commit
console.log('\n=== Example 1: Basic transaction with commit ===');
{
    const transaction = store.beginTransaction();

    // Add multiple quads in a transaction
    transaction.add(quad(subject, predicate1, literal('Alice')));
    transaction.add(quad(subject, predicate2, literal('30', namedNode('http://www.w3.org/2001/XMLSchema#integer'))));

    // Commit the transaction
    transaction.commit();

    console.log('Store size after commit:', store.size); // Should be 2
    console.log('Has name quad:', store.has(quad(subject, predicate1, literal('Alice'))));
}

// Example 2: Transaction rollback (not calling commit)
console.log('\n=== Example 2: Transaction rollback (implicit) ===');
{
    const initialSize = store.size;
    const transaction = store.beginTransaction();

    // Add a quad
    const tempQuad = quad(subject, namedNode('http://example.com/temp'), literal('temp'));
    transaction.add(tempQuad);

    // Don't commit - transaction is dropped/rolled back
    // (In JavaScript, just let it go out of scope)

    console.log('Store size after rollback:', store.size); // Should be same as before
    console.log('Has temp quad:', store.has(tempQuad)); // Should be false
}

// Example 3: Atomic update - delete and add
console.log('\n=== Example 3: Atomic update ===');
{
    const transaction = store.beginTransaction();

    // Delete old value
    transaction.delete(quad(subject, predicate2, literal('30', namedNode('http://www.w3.org/2001/XMLSchema#integer'))));

    // Add new value
    transaction.add(quad(subject, predicate2, literal('31', namedNode('http://www.w3.org/2001/XMLSchema#integer'))));

    // Commit atomically - both operations succeed or both fail
    transaction.commit();

    console.log('Has old age:', store.has(quad(subject, predicate2, literal('30', namedNode('http://www.w3.org/2001/XMLSchema#integer'))))); // false
    console.log('Has new age:', store.has(quad(subject, predicate2, literal('31', namedNode('http://www.w3.org/2001/XMLSchema#integer'))))); // true
}

// Example 4: Error handling
console.log('\n=== Example 4: Error handling ===');
{
    const transaction = store.beginTransaction();
    transaction.commit();

    try {
        // This should throw because transaction is already committed
        transaction.commit();
    } catch (error) {
        console.log('Caught expected error:', error.message);
    }

    try {
        // This should also throw
        transaction.add(quad(subject, predicate1, literal('test')));
    } catch (error) {
        console.log('Caught expected error:', error.message);
    }
}

console.log('\nFinal store size:', store.size);
