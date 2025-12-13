# PaymentRequestAcceptRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**jwt** | Option<**String**> | JWT for Ledger API authentication | [optional]
**buyer_party_id** | **String** | Buyer party ID | 
**buyer_private_key** | **String** | Base58-encoded Ed25519 private key | 
**request_cid** | **String** | Contract ID of AdvancedPaymentRequest | 
**amulet_cids** | **Vec<String>** | Amulet contract IDs to use as funds | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


