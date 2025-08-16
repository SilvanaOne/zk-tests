# \DefaultApi

All URIs are relative to *https://dhctq4vocgpmdbp5so7jfql26q0ubzms.lambda-url.us-east-1.on.aws*

Method | HTTP request | Description
------------- | ------------- | -------------
[**add_numbers**](DefaultApi.md#add_numbers) | **POST** /add | Add two numbers
[**multiply_numbers**](DefaultApi.md#multiply_numbers) | **POST** /multiply | Multiply two numbers



## add_numbers

> models::MathResponse add_numbers(math_request)
Add two numbers

Calculates the sum of two unsigned 64-bit integers

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**math_request** | [**MathRequest**](MathRequest.md) |  | [required] |

### Return type

[**models::MathResponse**](MathResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## multiply_numbers

> models::MathResponse multiply_numbers(math_request)
Multiply two numbers

Calculates the product of two unsigned 64-bit integers

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**math_request** | [**MathRequest**](MathRequest.md) |  | [required] |

### Return type

[**models::MathResponse**](MathResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

