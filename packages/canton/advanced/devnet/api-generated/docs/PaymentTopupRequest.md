# PaymentTopupRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**jwt** | Option<**String**> | JWT for Ledger API authentication | [optional]
**buyer_party_id** | **String** | Buyer party ID | 
**buyer_private_key** | **String** | Base58-encoded Ed25519 private key | 
**payment_cid** | **String** |  | 
**amount** | **String** | Amount to add | 
**new_expires** | Option<**String**> | New expiry time (ISO 8601) | [optional]
**amulet_cids** | **Vec<String>** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


